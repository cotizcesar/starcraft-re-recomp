import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

import java.io.FileWriter;
import java.io.PrintWriter;

/**
 * Disassembles and creates a function at each start address (in the session
 * only; run with -readOnly so nothing is saved), then decompiles it.
 * Usage: ... -postScript DecompileAt.java <outDir> [noreturn=a,b,...] <start1> ...
 * `noreturn=` clears Ghidra's (mis-analysed) no-return flag on those functions
 * first, so callers decompile past them.  `redo=N` then clears N bytes of
 * listing at each start and re-disassembles it (fall-through after those
 * calls was cut at the original disassembly).
 */
public class DecompileAt extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        int first = 1;
        if (args.length > 1 && args[1].startsWith("noreturn=")) {
            if (args[1].equals("noreturn=all")) {
                int n = 0;
                for (Function nf : currentProgram.getFunctionManager().getFunctions(true)) {
                    if (nf.hasNoReturn() && !nf.isExternal() && !nf.getName().contains("security")
                            && !nf.getName().toLowerCase().contains("throw")
                            && !nf.getName().toLowerCase().contains("abort")) {
                        nf.setNoReturn(false);
                        n++;
                    }
                }
                println("CLEARED_ALL " + n);
            }
            else for (String n : args[1].substring(9).split(",")) {
                Function nf = getFunctionAt(toAddr(n));
                if (nf != null) {
                    nf.setNoReturn(false);
                    println("CLEARED " + n);
                }
            }
            first = 2;
        }
        long redo = 0;
        if (args.length > first && args[first].startsWith("redo=")) {
            redo = Long.decode(args[first].substring(5));
            first++;
        }
        for (int i = first; i < args.length; i++) {
            Address a = toAddr(args[i]);
            if (redo > 0) {
                Function old = getFunctionContaining(a);
                if (old != null) {
                    removeFunction(old);
                }
                clearListing(a, a.add(redo - 1));
                disassemble(a);
            }
            Function f = getFunctionContaining(a);
            if (f == null) {
                disassemble(a);
                f = createFunction(a, null);
            }
            if (f == null) {
                println("NOFUNC " + args[i]);
                continue;
            }
            DecompileResults r = d.decompileFunction(f, 300, monitor);
            if (r == null || !r.decompileCompleted()) {
                println("FAIL " + args[i]);
                continue;
            }
            try (PrintWriter w = new PrintWriter(new FileWriter(args[0] + "/" + args[i] + ".c"))) {
                w.println("/* " + f.getName() + " @ " + f.getEntryPoint() + " */");
                w.println(r.getDecompiledFunction().getC());
            }
            println("OK " + args[i] + " -> " + f.getEntryPoint());
        }
    }
}
