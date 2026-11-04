import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;

import java.io.FileWriter;
import java.io.PrintWriter;

/**
 * Lists every reference to imported functions whose name contains one of the
 * given substrings (e.g. GetMessage PeekMessage DispatchMessage), with the
 * containing function.  Covers both the external symbol and its IAT thunk.
 * Read-only.  Usage (headless, no re-analysis):
 *   analyzeHeadless <projdir> <proj> -process -noanalysis -readOnly
 *     -scriptPath research/tools -postScript ImportRefs.java <out.csv> <name1> ...
 */
public class ImportRefs extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) {
            printerr("Usage: ImportRefs.java <out.csv> <name1> ...");
            return;
        }
        try (PrintWriter w = new PrintWriter(new FileWriter(args[0]))) {
            w.println("import,symbol_addr,ref_from,ref_type,ref_func,ref_func_entry");
            SymbolIterator it = currentProgram.getSymbolTable().getAllSymbols(true);
            int hits = 0;
            while (it.hasNext()) {
                Symbol s = it.next();
                String name = s.getName();
                boolean wanted = false;
                for (int i = 1; i < args.length; i++) {
                    if (name.contains(args[i])) {
                        wanted = true;
                    }
                }
                if (!wanted) {
                    continue;
                }
                for (Reference r : s.getReferences()) {
                    Address from = r.getFromAddress();
                    Function f = getFunctionContaining(from);
                    w.println(name + "," + s.getAddress() + "," + from + "," + r.getReferenceType()
                            + "," + (f == null ? "" : f.getName()) + ","
                            + (f == null ? "" : f.getEntryPoint().toString()));
                    hits++;
                }
            }
            println("IMPORT_REFS_DONE " + hits);
        }
    }
}
