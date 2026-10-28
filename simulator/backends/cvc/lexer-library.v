`timescale 1ps/1ps
// Uninstantiated library modules exercise the existing module skipper.
module skipped_endif(input d, output reg q);
`ifdef MD_ABSENT
  wire unused;
`endif
  always @(*) q = d;
endmodule
module skipped_else(input d, output reg q);
`ifdef MD_ABSENT
  wire unused;
`else
  always @(*) q = d;
`endif
endmodule
module skipped_elsif(input d, output reg q);
`ifdef MD_ABSENT
  wire unused;
`elsif MD_PRESENT
  always @(*) q = d;
`else
  wire other_unused;
`endif
endmodule
module skipped_nested(input d, output reg q);
`ifdef MD_ABSENT
`ifdef MD_PRESENT
  wire unused;
`endif
`endif
  always @(*) q = d;
endmodule
module lexer_cell(input [3:0] d, output reg [3:0] q);
`ifdef MD_ABSENT
  always @(*) q = 4'b0000;
`endif
  // The normal parser must restore its false context and still read attributes.
  (* keep = "true" *) wire [3:0] decorated = d;
  always @(*) q = decorated ^ 4'b1010;
endmodule
