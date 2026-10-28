`timescale 1ps/1ps
// Component vectors for the unchanged vendor primitives. These are digital
// semantics checks, not physical timing measurements or a processor model.
module primitive_vectors;
 reg [5:0] lut_input=6'bxxxxxx; wire lut5,lut6;
 LUT6_2 #(.INIT(64'h6996966996696996)) lut(.O5(lut5),.O6(lut6),.I0(lut_input[0]),.I1(lut_input[1]),.I2(lut_input[2]),.I3(lut_input[3]),.I4(lut_input[4]),.I5(lut_input[5]));
 reg [3:0] a=0,b=0; reg carry=0; wire [3:0] sum,co;
 CARRY4 add(.O(sum),.CO(co),.CI(carry),.CYINIT(1'b0),.DI(a),.S(a^b));
 reg clk=0,ce=0,d=0,r=0; wire q;
 FDRE #(.INIT(1'b1)) flop(.Q(q),.C(clk),.CE(ce),.D(d),.R(r));
 integer i,j,k,n=0; reg [4:0] expected;
 task pulse;
  begin #200 clk=1; #200; clk=0; #200; end
 endtask
 initial begin
  #200;
  if(q!==1'b1) $fatal(1,"FDRE initial global reset");
  #110000;
  // Drive the first vector after the vendor event processes have started.
  // Static declaration initialization does not create a later input event.
  $display("MD_SIM_LUT_BEFORE_FIRST_DRIVE O5=%b O6=%b",lut5,lut6);
  for(i=0;i<64;i=i+1) begin
   lut_input=i; #1;
   if(lut6!==^lut_input || lut5!==^lut_input[4:0]) $fatal(1,"LUT truth table %d O5=%b O6=%b input=%b expected5=%b expected6=%b",i,lut5,lut6,lut_input,^lut_input[4:0],^lut_input);
   n=n+1;
  end
  for(i=0;i<16;i=i+1) for(j=0;j<16;j=j+1) for(k=0;k<2;k=k+1) begin
   a=i; b=j; carry=k; expected=i+j+k; #1;
   if({co[3],sum}!==expected) $fatal(1,"CARRY arithmetic %d %d %d",i,j,k);
   n=n+1;
  end
  ce=0;d=0;pulse; if(q!==1'b1) $fatal(1,"disabled hold"); n=n+1;
  ce=1;d=0;pulse; if(q!==1'b0) $fatal(1,"enabled capture zero"); n=n+1;
  d=1;pulse; if(q!==1'b1) $fatal(1,"enabled capture one"); n=n+1;
  r=1;ce=0;pulse; if(q!==1'b0) $fatal(1,"reset overrides disable"); n=n+1;
  r=0;ce=1;d=1'bx;pulse; if(q!==1'bx) $fatal(1,"unknown capture"); n=n+1;
  r=1;pulse; if(q!==1'b0) $fatal(1,"reset clears unknown"); n=n+1;
  $display("MD_SIM_VENDOR_PRIMITIVES_PASS cases=%0d",n); $finish;
 end
endmodule
