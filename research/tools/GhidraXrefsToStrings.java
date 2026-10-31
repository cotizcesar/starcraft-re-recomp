import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSetView;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceManager;

import java.io.FileWriter;
import java.io.PrintWriter;
import java.util.HashSet;
import java.util.Set;

/**
 * Busca cadenas literales en memoria, lista sus xrefs y las funciones que las
 * referencian. Uso (headless, sin re-analisis):
 *   analyzeHeadless <projdir> <proj> -process <exe> -noanalysis
 *     -scriptPath ~/ReverseEngineering/tools -postScript GhidraXrefsToStrings.java <salida.csv> <cad1> [<cad2> ...]
 */
public class GhidraXrefsToStrings extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) {
            printerr("Uso: GhidraXrefsToStrings.java <salida.csv> <cad1> ...");
            return;
        }
        // NOTA: las cadenas se buscan tal cual (sin forzar NUL), porque en
        // memoria aparecen con sufijos (p.ej. "ChkLarvaStray: 1").
        Memory mem = currentProgram.getMemory();
        ReferenceManager rm = currentProgram.getReferenceManager();
        FunctionManager fm = currentProgram.getFunctionManager();
        AddressSetView init = mem.getLoadedAndInitializedAddressSet();
        try (PrintWriter w = new PrintWriter(new FileWriter(args[0]))) {
            w.println("string,hit_addr,ref_from,ref_func,ref_func_entry");
            for (int i = 1; i < args.length; i++) {
                byte[] pat = args[i].getBytes("ASCII");
                Address hit = mem.findBytes(init.getMinAddress(), pat, null,
                        true, monitor);
                int n = 0;
                while (hit != null && n < 200) {
                    for (Reference ref : rm.getReferencesTo(hit)) {
                        Address from = ref.getFromAddress();
                        Function fn = fm.getFunctionContaining(from);
                        String fnName = (fn == null) ? "" : fn.getName();
                        String fnEntry = (fn == null) ? ""
                                : "0x" + fn.getEntryPoint().toString().replace(" ", "");
                        w.println("\"" + args[i] + "\",0x"
                                + hit.toString().replace(" ", "") + ",0x"
                                + from.toString().replace(" ", "") + "," + fnName + ","
                                + fnEntry);
                    }
                    n++;
                    if (monitor.isCancelled()) {
                        break;
                    }
                    hit = mem.findBytes(hit.add(1), pat, null, true, monitor);
                }
                println("STR_DONE " + args[i] + " hits=" + n);
            }
        }
        println("XREF_BATCH_DONE");
    }
}
