`timescale 1ps/1ps
module lexer_top;
  reg [3:0] d;
  wire [3:0] q;
  integer count=0;
  lexer_cell dut(.d(d),.q(q));
  initial begin
    d=0; #1; if(q!==4'b1010) $fatal(1,"zero input"); count=count+1;
    d=5; #1; if(q!==4'b1111) $fatal(1,"known input"); count=count+1;
    d=4'bx1z0; #1; if(q!==4'bx1x0) $fatal(1,"four-state input"); count=count+1;
    $display("MD_CVC_LEXER_PASS cases=%0d time=%0t output=%b",count,$time,q);
    $finish;
  end
endmodule
