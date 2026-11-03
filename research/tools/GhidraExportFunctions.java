import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import ghidra.program.model.listing.FunctionManager;

import java.io.FileWriter;
import java.io.PrintWriter;

/**
 * Exports the current program's function inventory to CSV.
 * Usage (headless):
 *   analyzeHeadless &lt;projdir&gt; &lt;proj&gt; -process &lt;exe&gt; -noanalysis
 *     -scriptPath research/tools -postScript GhidraExportFunctions.java &lt;output.csv&gt;
 */
public class GhidraExportFunctions extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) {
            printerr("Usage: GhidraExportFunctions.java <output.csv>");
            return;
        }
        FunctionManager fm = currentProgram.getFunctionManager();
        int n = 0;
        try (PrintWriter w = new PrintWriter(new FileWriter(args[0]))) {
            w.println("entry,name,size,is_thunk,is_external");
            FunctionIterator it = fm.getFunctions(true);
            while (it.hasNext()) {
                Function fn = it.next();
                try {
                    String entry = "0x" + fn.getEntryPoint().toString().replace(" ", "");
                    w.println(entry + "," + fn.getName() + ","
                            + fn.getBody().getNumAddresses() + ","
                            + fn.isThunk() + "," + fn.isExternal());
                    n++;
                }
                catch (Exception e) {
                    // continue with the next function
                }
            }
        }
        println("FUNC_EXPORT_DONE functions=" + n + " program=" + currentProgram.getName());
    }
}
