`timescale 1ps/1ps
// This is a VPI event-region regression. It is not a miner or FPGA substitute.
module vpi_order;
  reg request=0;
  reg [63:0] request_sequence=0,operation=0,at_ps=0;
  reg [63:0] a0=0,a1=0,a2=0,a3=0,a4=0,a5=0,a6=0,a7=0,a8=0,a9=0,a10=0,a11=0;
  reg [31:0] first,second=0;
  // A declaration initializer runs before always sensitivity is installed.
  // This real time0 NBA starts the intended chain after active setup.
  initial first<=32'd0;
  wire [31:0] settled=(second^32'h55)+32'd3;
  reg [19:0] irq=0;
  wire [18:0] pins=settled[18:0];
  // Two NBA waves and a continuous expression must settle before receipt.
  always @(first) begin
    second<=first+32'd7;
    if(first==32'd9) irq<=20'b00000000000000000zx1;
  end
  initial $mdsim_init(request,request_sequence,operation,at_ps,
    a0,a1,a2,a3,a4,a5,a6,a7,a8,a9,a10,a11,irq,pins);
  always @(request) if(request_sequence!=0) begin
    if(operation==3) begin
      first<=32'd9;
      irq<=20'b00000000000000000xz1;
    end else if(operation!=5) $fatal(1,"unexpected event-fixture operation");
    $mdsim_complete(settled,2'b00,12'd0,64'd0,64'd0,1'b0);
`ifdef MD_DUPLICATE_COMPLETE
    $mdsim_complete(settled,2'b00,12'd0,64'd0,64'd0,1'b0);
`endif
  end
endmodule
