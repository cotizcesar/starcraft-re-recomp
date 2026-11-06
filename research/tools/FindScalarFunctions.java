import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.scalar.Scalar;

import java.io.FileWriter;
import java.io.PrintWriter;
import java.util.HashMap;
import java.util.Map;
import java.util.TreeMap;

/**
 * Lists the functions whose instructions use given scalar operands, with a
 * count per value, so functions combining several constants (e.g. a float
 * bit pattern and a table index) can be shortlisted.  Usage (headless):
 *   analyzeHeadless <projdir> <proj> -process <exe> -noanalysis -readOnly
 *     -scriptPath research/tools -postScript FindScalarFunctions.java
 *     <output.tsv> <hex1> [<hex2> ...]
 */
public class FindScalarFunctions extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        long[] want = new long[args.length - 1];
        for (int i = 1; i < args.length; i++) {
            want[i - 1] = Long.parseUnsignedLong(args[i].replace("0x", ""), 16);
        }
        Map<Long, long[]> hits = new TreeMap<>();
        Map<Long, String> names = new HashMap<>();
        InstructionIterator it = currentProgram.getListing().getInstructions(true);
        while (it.hasNext() && !monitor.isCancelled()) {
            Instruction ins = it.next();
            for (int op = 0; op < ins.getNumOperands(); op++) {
                for (Object o : ins.getOpObjects(op)) {
                    if (!(o instanceof Scalar)) {
                        continue;
                    }
                    long v = ((Scalar) o).getUnsignedValue() & 0xffffffffL;
                    for (int k = 0; k < want.length; k++) {
                        if (v == want[k]) {
                            Function f = getFunctionContaining(ins.getAddress());
                            long key = f == null ? -1 : f.getEntryPoint().getOffset();
                            hits.computeIfAbsent(key, x -> new long[want.length])[k]++;
                            if (f != null) {
                                names.put(key, f.getName());
                            }
                        }
                    }
                }
            }
        }
        try (PrintWriter w = new PrintWriter(new FileWriter(args[0]))) {
            StringBuilder h = new StringBuilder("function\tname");
            for (int i = 1; i < args.length; i++) {
                h.append('\t').append(args[i]);
            }
            w.println(h);
            for (Map.Entry<Long, long[]> e : hits.entrySet()) {
                StringBuilder row = new StringBuilder(String.format("0x%08x\t%s", e.getKey(),
                        names.getOrDefault(e.getKey(), "-")));
                for (long c : e.getValue()) {
                    row.append('\t').append(c);
                }
                w.println(row);
            }
        }
        println("SCALAR_DONE " + hits.size() + " functions");
    }
}
