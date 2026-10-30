// md-sim-hard sequence player. The Rust shell generator inserts this body
// inside module tb, after the pin vectors and PS7 port wires.
// Stimulus is two-state. Observation keeps four-state values, so an unknown
// circuit output stays visible as x in the trace.
localparam integer MAX_EVENTS = 262144;
localparam integer HANDSHAKE_LIMIT = 2000;
integer fd_in, fd_out, code, op, n_events, ev_i, k, k2, step, tag, count, tail;
integer until_cycle;
integer cyc;
reg running;
reg [31:0] addr, data;
reg [3:0] strobe;
reg [8*512-1:0] seq_path, out_path;
integer ev_cycle [0:MAX_EVENTS-1];
integer ev_pad [0:MAX_EVENTS-1];
integer ev_level [0:MAX_EVENTS-1];
reg [NOUT-1:0] last_out;
reg [19:0] last_irq;
wire clk = src_FCLKCLK[1];

initial begin
  cyc = 0; running = 0; ev_i = 0; n_events = 0;
  last_out = {NOUT{1'bx}}; last_irq = 20'hxxxxx;
end

// Cycle count: rising edges of the fabric clock after reset release.
always @(posedge clk) if (running) cyc = cyc + 1;

// Input pin changes take effect at the falling edge, half a cycle before
// the next rising edge samples them.
always @(negedge clk) if (running) begin
  while (ev_i < n_events && ev_cycle[ev_i] <= cyc) begin
    pin_in[ev_pad[ev_i]] = (ev_level[ev_i] != 0);
    ev_i = ev_i + 1;
  end
end

// Output pins and fabric interrupts, sampled in the middle of each cycle.
always @(negedge clk) if (running) begin
  #2;
  if (pin_out !== last_out) begin
    for (k2 = 0; k2 < NOUT; k2 = k2 + 1)
      if (pin_out[k2] !== last_out[k2]) $fwrite(fd_out, "T %0d %0d %b\n", cyc, k2, pin_out[k2]);
    last_out = pin_out;
  end
  if (ps_IRQF2P !== last_irq) begin
    $fwrite(fd_out, "Q %0d %h\n", cyc, ps_IRQF2P);
    last_irq = ps_IRQF2P;
  end
end

task timeout_exit(input integer idx);
  begin
    $fwrite(fd_out, "X %0d %0d\n", idx, cyc);
    $fclose(fd_out);
    $finish;
  end
endtask

// One AXI read. A handshake happens at the rising edge after the falling
// edge where both VALID and READY are high.
task do_read(input integer idx, input [31:0] a);
  integer s, w;
  begin
    @(negedge clk);
    src_MAXIGP0ARADDR = a; src_MAXIGP0ARID = idx % 4096; src_MAXIGP0ARLEN = 0;
    src_MAXIGP0ARSIZE = 2; src_MAXIGP0ARBURST = 1; src_MAXIGP0ARVALID = 1;
    src_MAXIGP0RREADY = 1;
    s = cyc; w = 0;
    #1;
    while (ps_MAXIGP0ARREADY !== 1'b1 && w < HANDSHAKE_LIMIT) begin @(negedge clk); #1; w = w + 1; end
    if (w >= HANDSHAKE_LIMIT) timeout_exit(idx);
    @(negedge clk); src_MAXIGP0ARVALID = 0; #1;
    while (ps_MAXIGP0RVALID !== 1'b1 && w < HANDSHAKE_LIMIT) begin @(negedge clk); #1; w = w + 1; end
    if (w >= HANDSHAKE_LIMIT) timeout_exit(idx);
    $fwrite(fd_out, "R %0d %0d %0d %h %b\n", idx, s, cyc, ps_MAXIGP0RDATA, ps_MAXIGP0RRESP);
    @(negedge clk); src_MAXIGP0RREADY = 0;
  end
endtask

// One AXI write with address and data offered together.
task do_write(input integer idx, input [31:0] a, input [31:0] d, input [3:0] st);
  integer s, w;
  reg aw_done, w_done;
  begin
    @(negedge clk);
    src_MAXIGP0AWADDR = a; src_MAXIGP0AWID = idx % 4096; src_MAXIGP0AWLEN = 0;
    src_MAXIGP0AWSIZE = 2; src_MAXIGP0AWBURST = 1; src_MAXIGP0AWVALID = 1;
    src_MAXIGP0WDATA = d; src_MAXIGP0WSTRB = st; src_MAXIGP0WID = idx % 4096;
    src_MAXIGP0WLAST = 1; src_MAXIGP0WVALID = 1; src_MAXIGP0BREADY = 1;
    s = cyc; w = 0; aw_done = 0; w_done = 0;
    while ((!aw_done || !w_done) && w < HANDSHAKE_LIMIT) begin
      #1;
      if (src_MAXIGP0AWVALID && ps_MAXIGP0AWREADY === 1'b1) aw_done = 1;
      if (src_MAXIGP0WVALID && ps_MAXIGP0WREADY === 1'b1) w_done = 1;
      @(negedge clk);
      if (aw_done) src_MAXIGP0AWVALID = 0;
      if (w_done) src_MAXIGP0WVALID = 0;
      w = w + 1;
    end
    if (w >= HANDSHAKE_LIMIT) timeout_exit(idx);
    #1;
    while (ps_MAXIGP0BVALID !== 1'b1 && w < HANDSHAKE_LIMIT) begin @(negedge clk); #1; w = w + 1; end
    if (w >= HANDSHAKE_LIMIT) timeout_exit(idx);
    $fwrite(fd_out, "W %0d %0d %0d %b\n", idx, s, cyc, ps_MAXIGP0BRESP);
    @(negedge clk); src_MAXIGP0BREADY = 0;
  end
endtask

initial begin
  if (!$value$plusargs("seq=%s", seq_path)) begin $display("missing +seq"); $finish; end
  if (!$value$plusargs("out=%s", out_path)) begin $display("missing +out"); $finish; end
  fd_in = $fopen(seq_path, "r");
  fd_out = $fopen(out_path, "w");
  if (fd_in == 0 || fd_out == 0) begin $display("cannot open sequence or output"); $finish; end
  code = $fscanf(fd_in, "%d", n_events);
  if (n_events > MAX_EVENTS) begin $display("too many events"); $finish; end
  for (k = 0; k < n_events; k = k + 1)
    code = $fscanf(fd_in, "%d %d %d", ev_cycle[k], ev_pad[k], ev_level[k]);
  src_FCLKRESETN = 4'h0; src_MAXIGP0ARESETN = 0;
  repeat (64) @(negedge clk);
  src_FCLKRESETN = 4'hf; src_MAXIGP0ARESETN = 1;
  running = 1;
  step = 0;
  forever begin
    code = $fscanf(fd_in, "%d", op);
    if (code != 1) op = 9;
    case (op)
      1: begin code = $fscanf(fd_in, "%h", addr); do_read(step, addr); end
      2: begin code = $fscanf(fd_in, "%h %h %h", addr, data, strobe); do_write(step, addr, data, strobe); end
      3: begin code = $fscanf(fd_in, "%d", count); repeat (count) @(negedge clk); end
      4: begin code = $fscanf(fd_in, "%d", until_cycle); while (cyc < until_cycle) @(negedge clk); end
      5: begin code = $fscanf(fd_in, "%d", tag); @(negedge clk); #3; $fwrite(fd_out, "K %0d %0d\n", tag, cyc); end
      default: begin
        tail = 0; code = $fscanf(fd_in, "%d", tail);
        repeat (tail) @(negedge clk);
        #3; $fwrite(fd_out, "E %0d\n", cyc);
        $fclose(fd_out);
        $finish;
      end
    endcase
    step = step + 1;
  end
end
