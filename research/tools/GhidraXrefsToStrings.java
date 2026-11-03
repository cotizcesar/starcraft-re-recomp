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
 * Searches memory for literal strings, lists their xrefs and the functions that
 * reference them. Usage (headless, no re-analysis):
 *   analyzeHeadless <projdir> <proj> -process <exe> -noanalysis
 *     -scriptPath research/tools -postScript GhidraXrefsToStrings.java <output.csv> <str1> [<str2> ...]
 */
public class GhidraXrefsToStrings extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) {
            printerr("Usage: GhidraXrefsToStrings.java <output.csv> <str1> ...");
            return;
        }
        // NOTE: strings are searched as-is (without forcing NUL), because in
        // memory they appear with suffixes (e.g. "ChkLarvaStray: 1").
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
