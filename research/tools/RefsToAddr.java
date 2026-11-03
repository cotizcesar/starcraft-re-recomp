import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceManager;

import java.io.FileWriter;
import java.io.PrintWriter;

/**
 * Lists all references to the given addresses and their containing function.
 * Usage (headless, no re-analysis):
 *   analyzeHeadless <projdir> <proj> -process <exe> -noanalysis
 *     -scriptPath .../tools -postScript RefsToAddr.java <output.csv> <addr1> ...
 */
public class RefsToAddr extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) {
            printerr("Usage: RefsToAddr.java <output.csv> <addr1> ...");
            return;
        }
        ReferenceManager rm = currentProgram.getReferenceManager();
        FunctionManager fm = currentProgram.getFunctionManager();
        try (PrintWriter w = new PrintWriter(new FileWriter(args[0]))) {
            w.println("target,ref_from,ref_type,ref_func,ref_func_entry");
            for (int i = 1; i < args.length; i++) {
                Address target = currentProgram.getAddressFactory()
                        .getDefaultAddressSpace().getAddress(args[i]);
                int n = 0;
                for (Reference ref : rm.getReferencesTo(target)) {
                    Address from = ref.getFromAddress();
                    Function fn = fm.getFunctionContaining(from);
                    String fnName = (fn == null) ? "" : fn.getName();
                    String fnEntry = (fn == null) ? ""
                            : "0x" + fn.getEntryPoint().toString().replace(" ", "");
                    w.println(args[i] + ",0x" + from.toString().replace(" ", "") + ","
                            + ref.getReferenceType() + "," + fnName + "," + fnEntry);
                    if (++n > 500) {
                        break;
                    }
                }
                println("REFS_DONE " + args[i] + " n=" + n);
            }
        }
        println("REFS_BATCH_DONE");
    }
}
