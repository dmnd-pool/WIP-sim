// The C VPI adapter writes this batch in future read-write synchronization,
// after old clock edges and their active/NBA work at the requested timestamp.
// It samples the result after all HDL delta cycles, in read-only synchronization.
reg md_request=0;
reg [63:0] md_sequence=0,md_operation=0,md_requested_ps=0;
reg [63:0] md_arg0=0,md_arg1=0,md_arg2=0,md_arg3=0;
reg [63:0] md_arg4=0,md_arg5=0,md_arg6=0,md_arg7=0;
reg [63:0] md_arg8=0,md_arg9=0,md_arg10=0,md_arg11=0;
reg [31:0] md_reply_data=0;
reg [1:0] md_reply_response=0;
reg [11:0] md_reply_id=0;
reg [63:0] md_cycles=0,md_axi_edges=0,md_before_edges=0;
reg [63:0] md_period_q32[0:3],md_clock_epoch[0:3];
reg [127:0] md_next_scaled[0:3];
reg [63:0] md_next_clock_ps=0;
reg md_next_clock_known=0;
integer md_index;
reg [63:0] md_new_period;

// Units are picoseconds multiplied by2^33. A half period is the original
// QEMU Q32-nanosecond period multiplied by1000. Round each absolute edge up
// to1ps, without repeated period rounding. This is a digital phase policy.
function automatic [63:0] md_edge_ps(input [127:0] scaled);
  reg [127:0] rounded;
  begin
    rounded=(scaled+128'd8589934591)>>33;
    if(rounded>128'hffffffffffffffff) $fatal(1,"MDPL clock timestamp overflow");
    md_edge_ps=rounded[63:0];
  end
endfunction

initial begin
  for(md_index=0;md_index<4;md_index=md_index+1) begin
    md_period_q32[md_index]=0;
    md_clock_epoch[md_index]=0;
    md_next_scaled[md_index]=0;
  end
  $mdsim_init(md_request,md_sequence,md_operation,md_requested_ps,
    md_arg0,md_arg1,md_arg2,md_arg3,md_arg4,md_arg5,md_arg6,md_arg7,
    md_arg8,md_arg9,md_arg10,md_arg11,md_irq,md_outputs);
end

genvar md_channel;
generate for(md_channel=0;md_channel<4;md_channel=md_channel+1) begin: md_clock
  reg [63:0] epoch,edge_ps;
  initial forever begin
    while((md_period_q32[md_channel]!=0)!==1'b1) @(md_period_q32[md_channel]);
    epoch=md_clock_epoch[md_channel];
    edge_ps=md_edge_ps(md_next_scaled[md_channel]);
    if(edge_ps<=$time) $fatal(1,"MDPL non-future clock edge");
    fork: pending_edge
      begin
        #(edge_ps-$time);
        if(md_clock_epoch[md_channel]==epoch) begin
          src_FCLKCLK[md_channel]=~src_FCLKCLK[md_channel];
          md_next_scaled[md_channel]=md_next_scaled[md_channel]+
            ({64'd0,md_period_q32[md_channel]}*128'd1000);
        end
        disable pending_edge;
      end
      begin
        while((md_clock_epoch[md_channel]!=epoch)!==1'b1) @(md_clock_epoch[md_channel]);
        disable pending_edge;
      end
    join
  end
end endgenerate

integer md_clock_index;
reg [63:0] md_candidate_edge;
always @(md_period_q32[0] or md_period_q32[1] or md_period_q32[2] or md_period_q32[3]
    or md_next_scaled[0] or md_next_scaled[1] or md_next_scaled[2] or md_next_scaled[3]) begin
  md_next_clock_known=0;md_next_clock_ps=0;
  for(md_clock_index=0;md_clock_index<4;md_clock_index=md_clock_index+1) begin
    if(md_period_q32[md_clock_index]!=0) begin
      md_candidate_edge=md_edge_ps(md_next_scaled[md_clock_index]);
      if(!md_next_clock_known || md_candidate_edge<md_next_clock_ps)
        md_next_clock_ps=md_candidate_edge;
      md_next_clock_known=1;
    end
  end
end

always @(posedge ps_MAXIGP0ACLK) begin
  if(md_axi_edges==64'hffffffffffffffff) $fatal(1,"MDPL clock count overflow");
  md_axi_edges=md_axi_edges+1;
end

always @(md_request) begin
  // The initial0 assignment is not a command. The sequence is written first.
  if(md_sequence!=0) begin
    if($time!==md_requested_ps) $fatal(1,"MDPL command time differs");
    md_reply_data=0;md_reply_response=0;md_reply_id=0;md_cycles=0;
    case(md_operation)
      1,2: begin
        // Old clock observers already settled before this batch. Handshakes
        // start on a later negedge; no virtual or delta delay is inserted.
        md_before_edges=md_axi_edges;
        if(md_operation==1) begin
          read_word(md_arg0[31:0],md_sequence[11:0]);
          md_reply_data=last_read;
        end else begin
          write_word(md_arg0[31:0],md_arg1[31:0],md_arg2[3:0],md_sequence[11:0]);
        end
        md_reply_id=last_id;md_reply_response=last_response;
        md_cycles=md_axi_edges-md_before_edges;
        if(md_cycles==0 || md_cycles>1024) $fatal(1,"MDPL AXI cycle bound");
      end
      3: begin end // Actual advance finished when this scheduled task runs.
      4: begin
        for(md_index=0;md_index<4;md_index=md_index+1) begin
          case(md_index)
            0:md_new_period=md_arg0;1:md_new_period=md_arg1;
            2:md_new_period=md_arg2;3:md_new_period=md_arg3;
          endcase
          if(md_arg8[md_index] && md_new_period!=md_period_q32[md_index]) begin
            if(md_clock_epoch[md_index]==64'hffffffffffffffff)
              $fatal(1,"MDPL clock epoch overflow");
            // First enable starts low. Later changes preserve the current
            // level and begin a new half-period phase. Gating holds the level.
            if(md_clock_epoch[md_index]==0) src_FCLKCLK[md_index]=0;
            md_clock_epoch[md_index]=md_clock_epoch[md_index]+1;
            md_period_q32[md_index]=md_new_period;
            md_next_scaled[md_index]=({64'd0,$time}<<33)+
              ({64'd0,md_new_period}*128'd1000);
          end
        end
        if(md_arg8[4])src_FCLKRESETN=md_arg4[3:0];
        if(md_arg8[5])src_MAXIGP0ARESETN=md_arg5[0];
        if(md_arg8[6]) begin
          for(md_index=0;md_index<16;md_index=md_index+1) begin
            if(md_arg7[md_index]) begin
              case({md_arg6[32+md_index],md_arg6[md_index]})
                2'b00:md_inputs[md_index]=0;
                2'b01:md_inputs[md_index]=1;
                2'b10:md_inputs[md_index]=1'bz;
                2'b11:md_inputs[md_index]=1'bx;
              endcase
            end
          end
        end
      end
      5: begin end // Only the VPI read-only callback finishes the process.
      default:$fatal(1,"MDPL unsupported operation");
    endcase
    $mdsim_complete(md_reply_data,md_reply_response,md_reply_id,md_cycles,
      md_next_clock_ps,md_next_clock_known);
  end
end
