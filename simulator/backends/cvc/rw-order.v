`timescale 1ps/1ps
// Independent scheduler oracle, not an FPGA or miner model.
module rw_order;
  reg clock = 0;
  reg incoming = 0;
  reg edge_seen = 0;
  reg sampled = 1'bx;
  always @(posedge clock) edge_seen <= 1;
  // The second NBA wave must consume the old input before the future RW drive.
  always @(edge_seen) if (edge_seen) sampled <= incoming;
  initial begin
    #5 clock = 1;
    #5 $display("MD_RW_ORACLE_MISSING_CALLBACK");
    $finish;
  end
endmodule
