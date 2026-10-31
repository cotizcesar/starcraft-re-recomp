import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;

import java.io.FileWriter;
import java.io.PrintWriter;

/**
 * Decompila una lista de direcciones y vuelca el pseudocodigo a <salidaDir>/<addr>.c.
 * Uso (headless, con analisis ya guardado):
 *   analyzeHeadless <projdir> <proj> -process <exe> -noanalysis
 *     -scriptPath ~/ReverseEngineering/tools -postScript GhidraDecompileBatch.java <salidaDir> <addr1> [<addr2> ...]
 */
public class GhidraDecompileBatch extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) {
            printerr("Uso: GhidraDecompileBatch.java <salidaDir> <addr1> ...");
            return;
        }
        String outDir = args[0];
        DecompInterface decomp = new DecompInterface();
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
                DecompileResults res = decomp.decompileFunction(fn, 300, monitor);
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
                    println("FAIL " + args[i]);
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
