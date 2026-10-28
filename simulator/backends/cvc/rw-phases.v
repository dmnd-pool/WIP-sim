`timescale 1ps/1ps
// Scheduler oracle only. Each build deliberately selects a different NBA path.
module rw_phases;
  reg clock = 0;
  reg incoming = 0;
  reg edge_seen = 0;
  reg sampled = 1'bx;
  reg echo_first = 0;
  reg echo_second = 0;
`ifdef MD_NO_NBA
  always @(posedge clock) begin edge_seen = 1; sampled = incoming; end
  always @(incoming) echo_first = incoming;
  always @(echo_first) echo_second = echo_first;
`else
`ifdef MD_DELAYED_NBA
  always @(posedge clock) edge_seen <= #3 1;
  initial #2 clock = 1;
`else
  always @(posedge clock) edge_seen <= 1;
  initial #5 clock = 1;
`endif
  always @(edge_seen) if (edge_seen) sampled <= incoming;
  always @(incoming) echo_first <= incoming;
  always @(echo_first) echo_second <= echo_first;
`endif
`ifdef MD_NO_NBA
  initial #5 clock = 1;
`endif
  // No future fallback event keeps the simulation alive after the test edge.
  // The owner must see the exact callback receipt, even when the process exits0.
endmodule
