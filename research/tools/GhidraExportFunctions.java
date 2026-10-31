import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import ghidra.program.model.listing.FunctionManager;

import java.io.FileWriter;
import java.io.PrintWriter;

/**
 * Exporta el inventario de funciones del programa actual a CSV.
 * Uso (headless):
 *   analyzeHeadless &lt;projdir&gt; &lt;proj&gt; -process &lt;exe&gt; -noanalysis
 *     -scriptPath ~/ReverseEngineering/tools -postScript GhidraExportFunctions.java &lt;salida.csv&gt;
 */
public class GhidraExportFunctions extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) {
            printerr("Uso: GhidraExportFunctions.java <salida.csv>");
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
                    // continua con la siguiente funcion
                }
            }
        }
        println("FUNC_EXPORT_DONE functions=" + n + " program=" + currentProgram.getName());
    }
}
