// Draft task body. The Rust generator must bind only declared PS7 ports.
// All constants in the eventual call sequence require the exact ELF audit.
integer transactions=0;
reg [31:0] last_read;

task automatic read_word(input [31:0] address, input [11:0] tag);
  integer wait_cycles, hold_cycles;
  reg [31:0] saved;
  reg [11:0] saved_id;
  reg [1:0] saved_response;
  reg saved_last;
  begin
    @(negedge ps_MAXIGP0ACLK);
    src_MAXIGP0ARADDR=address; src_MAXIGP0ARID=tag;
    src_MAXIGP0ARLEN=0; src_MAXIGP0ARSIZE=2;
    src_MAXIGP0ARBURST=1; src_MAXIGP0ARVALID=1;
    src_MAXIGP0RREADY=0;
    wait_cycles=0;
    begin : address_wait
      forever begin
        @(posedge ps_MAXIGP0ACLK);
        if (ps_MAXIGP0ARREADY===1'b1) disable address_wait;
        if (ps_MAXIGP0ARREADY!==1'b0) $fatal(1,"unknown ARREADY addr=%08x value=%b",address,ps_MAXIGP0ARREADY);
        wait_cycles=wait_cycles+1;
        if(wait_cycles==256) $fatal(1,"AR handshake timeout addr=%08x",address);
      end
    end
    @(negedge ps_MAXIGP0ACLK); src_MAXIGP0ARVALID=0;
    wait_cycles=0;
    begin : response_wait
      forever begin
        @(posedge ps_MAXIGP0ACLK);
        if(ps_MAXIGP0RVALID===1'b1) disable response_wait;
        if(ps_MAXIGP0RVALID!==1'b0) $fatal(1,"unknown RVALID addr=%08x value=%b",address,ps_MAXIGP0RVALID);
        wait_cycles=wait_cycles+1;
        if(wait_cycles==256) $fatal(1,"R handshake timeout addr=%08x",address);
      end
    end
    saved=ps_MAXIGP0RDATA; saved_id=ps_MAXIGP0RID;
    saved_response=ps_MAXIGP0RRESP; saved_last=ps_MAXIGP0RLAST;
    $display("AXI_READ_OBS addr=%08x id=%03x data=%08x resp=%b last=%b",address,saved_id,saved,saved_response,saved_last);
    if(saved_id!==tag || saved_response!==2'b00 || saved_last!==1'b1 || (^saved)===1'bx)
      $fatal(1,"invalid or unknown read response addr=%08x",address);
    for(hold_cycles=0;hold_cycles<3;hold_cycles=hold_cycles+1) begin
      @(posedge ps_MAXIGP0ACLK);
      if(ps_MAXIGP0RVALID!==1'b1 || ps_MAXIGP0RDATA!==saved || ps_MAXIGP0RID!==saved_id
        || ps_MAXIGP0RRESP!==saved_response || ps_MAXIGP0RLAST!==saved_last)
        $fatal(1,"R response changed while blocked addr=%08x",address);
    end
    @(negedge ps_MAXIGP0ACLK); src_MAXIGP0RREADY=1;
    @(posedge ps_MAXIGP0ACLK);
    if(ps_MAXIGP0RVALID!==1'b1 || ps_MAXIGP0RDATA!==saved) $fatal(1,"R response disappeared before acceptance");
    @(negedge ps_MAXIGP0ACLK); src_MAXIGP0RREADY=0;
    last_read=saved; transactions=transactions+1;
  end
endtask

task automatic write_word(input [31:0] address,input [31:0] value,input [3:0] strobe,input [11:0] tag);
  integer wait_cycles, hold_cycles;
  reg address_done,data_done;
  reg [11:0] saved_id;
  reg [1:0] saved_response;
  begin
    @(negedge ps_MAXIGP0ACLK);
    src_MAXIGP0AWADDR=address;src_MAXIGP0AWID=tag;src_MAXIGP0AWLEN=0;
    src_MAXIGP0AWSIZE=2;src_MAXIGP0AWBURST=1;src_MAXIGP0AWVALID=1;
    src_MAXIGP0WDATA=value;src_MAXIGP0WSTRB=strobe;src_MAXIGP0WID=tag;
    src_MAXIGP0WLAST=1;src_MAXIGP0WVALID=1;src_MAXIGP0BREADY=0;
    address_done=0;data_done=0;wait_cycles=0;
    while(!address_done || !data_done) begin
      @(posedge ps_MAXIGP0ACLK);
      if(!address_done) begin
        if(ps_MAXIGP0AWREADY===1'b1) address_done=1;
        else if(ps_MAXIGP0AWREADY!==1'b0) $fatal(1,"unknown AWREADY addr=%08x",address);
      end
      if(!data_done) begin
        if(ps_MAXIGP0WREADY===1'b1) data_done=1;
        else if(ps_MAXIGP0WREADY!==1'b0) $fatal(1,"unknown WREADY addr=%08x",address);
      end
      wait_cycles=wait_cycles+1;
      if(wait_cycles==256) $fatal(1,"AW/W handshake timeout addr=%08x",address);
      @(negedge ps_MAXIGP0ACLK);
      if(address_done) src_MAXIGP0AWVALID=0;
      if(data_done) src_MAXIGP0WVALID=0;
    end
    wait_cycles=0;
    begin : response_wait
      forever begin
        @(posedge ps_MAXIGP0ACLK);
        if(ps_MAXIGP0BVALID===1'b1) disable response_wait;
        if(ps_MAXIGP0BVALID!==1'b0) $fatal(1,"unknown BVALID addr=%08x",address);
        wait_cycles=wait_cycles+1;
        if(wait_cycles==256) $fatal(1,"B handshake timeout addr=%08x",address);
      end
    end
    saved_id=ps_MAXIGP0BID;saved_response=ps_MAXIGP0BRESP;
    $display("AXI_WRITE_OBS addr=%08x id=%03x data=%08x strobe=%x resp=%b",address,saved_id,value,strobe,saved_response);
    if(saved_id!==tag || saved_response!==2'b00) $fatal(1,"invalid write response addr=%08x",address);
    for(hold_cycles=0;hold_cycles<3;hold_cycles=hold_cycles+1) begin
      @(posedge ps_MAXIGP0ACLK);
      if(ps_MAXIGP0BVALID!==1'b1 || ps_MAXIGP0BID!==saved_id || ps_MAXIGP0BRESP!==saved_response)
        $fatal(1,"B response changed while blocked addr=%08x",address);
    end
    @(negedge ps_MAXIGP0ACLK);src_MAXIGP0BREADY=1;
    @(posedge ps_MAXIGP0ACLK);
    if(ps_MAXIGP0BVALID!==1'b1 || ps_MAXIGP0BID!==saved_id || ps_MAXIGP0BRESP!==saved_response)
      $fatal(1,"B response disappeared before acceptance");
    @(negedge ps_MAXIGP0ACLK);src_MAXIGP0BREADY=0;
    transactions=transactions+1;
  end
endtask
