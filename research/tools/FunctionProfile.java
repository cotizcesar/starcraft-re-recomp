import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.StringDataInstance;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.symbol.Reference;

import java.io.FileWriter;
import java.io.PrintWriter;
import java.util.Map;
import java.util.TreeMap;

/**
 * Structural profile of one function, for bodies too large to decompile:
 * size, callers, every call target (with count), indirect calls, and the
 * strings / data it references, in address order.  Read-only.  Usage:
 *   analyzeHeadless <projdir> <proj> -process -noanalysis -readOnly
 *     -scriptPath research/tools -postScript FunctionProfile.java <out.txt> <addr>
 */
public class FunctionProfile extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) {
            printerr("Usage: FunctionProfile.java <out.txt> <addr>");
            return;
        }
        Address addr = toAddr(args[1]);
        Function fn = getFunctionContaining(addr);
        if (fn == null) {
            println("NOFUNC " + args[1]);
            return;
        }
        Map<String, Integer> callees = new TreeMap<>();
        int indirect = 0;
        int instructions = 0;
        StringBuilder strings = new StringBuilder();
        StringBuilder data = new StringBuilder();
        InstructionIterator it = currentProgram.getListing().getInstructions(fn.getBody(), true);
        while (it.hasNext()) {
            Instruction ins = it.next();
            instructions++;
            boolean isCall = ins.getFlowType().isCall();
            if (isCall && ins.getFlowType().isComputed()) {
                indirect++;
            }
            for (Reference r : ins.getReferencesFrom()) {
                Address to = r.getToAddress();
                if (r.getReferenceType().isCall()) {
                    Function t = getFunctionAt(to);
                    String key = to + " " + (t == null ? "?" : t.getName());
                    callees.merge(key, 1, Integer::sum);
                    continue;
                }
                if (!r.getReferenceType().isData()) {
                    continue;
                }
                Data d = getDataAt(to);
                if (d != null && d.hasStringValue()) {
                    String s = StringDataInstance.getStringDataInstance(d).getStringValue();
                    strings.append(ins.getAddress()).append(" -> ").append(to).append(" \"")
                            .append(s == null ? "" : s.replace("\n", "\\n")).append("\"\n");
                }
                else {
                    data.append(ins.getAddress()).append(" ").append(r.getReferenceType())
                            .append(" ").append(to).append("\n");
                }
            }
        }
        try (PrintWriter w = new PrintWriter(new FileWriter(args[0]))) {
            w.println("function " + fn.getName() + " @ " + fn.getEntryPoint() + " bytes="
                    + fn.getBody().getNumAddresses() + " instructions=" + instructions
                    + " indirect_calls=" + indirect);
            w.println("callers:");
            for (Function c : fn.getCallingFunctions(monitor)) {
                w.println("  " + c.getEntryPoint() + " " + c.getName());
            }
            w.println("callees (" + callees.size() + " distinct):");
            for (Map.Entry<String, Integer> e : callees.entrySet()) {
                w.println("  " + e.getKey() + " x" + e.getValue());
            }
            w.println("strings:");
            w.print(strings);
            w.println("data refs:");
            w.print(data);
        }
        println("PROFILE_DONE " + fn.getEntryPoint());
    }
}
