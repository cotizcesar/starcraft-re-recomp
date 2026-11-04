import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;

import java.io.FileWriter;
import java.io.PrintWriter;

/**
 * Decompiles a list of addresses and dumps the pseudocode to <outputDir>/<addr>.c.
 * Usage (headless, with analysis already saved):
 *   analyzeHeadless <projdir> <proj> -process <exe> -noanalysis
 *     -scriptPath research/tools -postScript GhidraDecompileBatch.java <outputDir> <addr1> [<addr2> ...]
 */
public class GhidraDecompileBatch extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) {
            printerr("Usage: GhidraDecompileBatch.java <outputDir> <addr1> ...");
            return;
        }
        String outDir = args[0];
        DecompInterface decomp = new DecompInterface();
        // GHIDRA_DECOMPILE_PAYLOAD_MB / GHIDRA_DECOMPILE_MAX_INSTR lift the
        // decompiler's output and flow limits (large init functions).
        ghidra.app.decompiler.DecompileOptions opts = new ghidra.app.decompiler.DecompileOptions();
        String payload = System.getenv("GHIDRA_DECOMPILE_PAYLOAD_MB");
        if (payload != null) {
            opts.setMaxPayloadMBytes(Integer.parseInt(payload.trim()));
        }
        String maxInstr = System.getenv("GHIDRA_DECOMPILE_MAX_INSTR");
        if (maxInstr != null) {
            opts.setMaxInstructions(Integer.parseInt(maxInstr.trim()));
        }
        decomp.setOptions(opts);
        decomp.openProgram(currentProgram);
        FunctionManager fm = currentProgram.getFunctionManager();
        int ok = 0, fail = 0;
        for (int i = 1; i < args.length; i++) {
            Address addr = currentProgram.getAddressFactory()
                    .getDefaultAddressSpace().getAddress(args[i]);
            Function fn = fm.getFunctionAt(addr);
            if (fn == null) {
                fn = fm.getFunctionContaining(addr);
            }
            if (fn == null) {
                println("NOFUNC " + args[i]);
                fail++;
                continue;
            }
            try {
                // GHIDRA_DECOMPILE_TIMEOUT (seconds) for very large functions.
                int timeout = 300;
                String env = System.getenv("GHIDRA_DECOMPILE_TIMEOUT");
                if (env != null) {
                    timeout = Integer.parseInt(env.trim());
                }
                DecompileResults res = decomp.decompileFunction(fn, timeout, monitor);
                String name = "0x" + fn.getEntryPoint().toString().replace(" ", "");
                if (res != null && res.decompileCompleted()) {
                    try (PrintWriter w = new PrintWriter(new FileWriter(outDir + "/" + name + ".c"))) {
                        w.println("/* " + fn.getName() + " @ " + name + " size="
                                + fn.getBody().getNumAddresses() + " */");
                        w.println(res.getDecompiledFunction().getC());
                    }
                    println("OK " + name);
                    ok++;
                }
                else {
                    println("FAIL " + args[i] + " size=" + fn.getBody().getNumAddresses()
                            + " error=" + (res == null ? "null" : res.getErrorMessage()));
                    fail++;
                }
            }
            catch (Exception e) {
                println("EXC " + args[i] + " " + e.getMessage());
                fail++;
            }
        }
        decomp.dispose();
        println("DECOMPILE_BATCH_DONE ok=" + ok + " fail=" + fail);
    }
}
