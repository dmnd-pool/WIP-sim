`timescale 1ps/1ps
// Component oracle only. The actual live clock/control code is appended.
module clock_order;
  reg [3:0] src_FCLKCLK=0,src_FCLKRESETN=0;
  reg src_MAXIGP0ARESETN=0;
  wire ps_MAXIGP0ACLK=src_FCLKCLK[1];
  reg [15:0] md_inputs=0;
  wire [19:0] md_irq=0;
  reg [3:0] sampled_edges=0;
  reg sampled_input=0,sampled_reset=0;
  wire [18:0] md_outputs={12'd0,sampled_input,sampled_reset,sampled_edges,src_FCLKCLK[1]};
`ifdef MD_UNKNOWN_CLOCK
  // An unknown period must not enable a clock. Active initialization precedes
  // this time0 NBA. The valid command arrives only after the later zero write.
  initial begin
    md_period_q32[1]<=64'hxxxxxxxxxxxxxxxx;
    #100;md_period_q32[1]=0;
  end
`endif
  always @(posedge ps_MAXIGP0ACLK) begin
    sampled_edges<=sampled_edges+1;
    sampled_input<=md_inputs[0];
    sampled_reset<=src_FCLKRESETN[0];
  end
  reg [31:0] last_read=0;
  reg [11:0] last_id=0;
  reg [1:0] last_response=0;
  task read_word(input [31:0] address,input [11:0] id);
    $fatal(1,"clock fixture does not implement AXI");
  endtask
  task write_word(input [31:0] address,input [31:0] data,input [3:0] strobe,input [11:0] id);
    $fatal(1,"clock fixture does not implement AXI");
  endtask
