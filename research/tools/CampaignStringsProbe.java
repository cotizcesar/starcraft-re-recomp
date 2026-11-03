import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSetView;
import ghidra.program.model.mem.Memory;

import java.io.FileWriter;
import java.io.PrintWriter;

/**
 * String probe for the campaign selection screen (glucmpgn).
 * Dumps every printable ASCII string containing any of the patterns,
 * with address and content, to identify RTTI classes (Screen/Campaign),
 * resource tables and FLC names present in the menu dump.
 *
 * Usage (headless, no re-analysis):
 *   analyzeHeadless <projdir> <proj> -process sc_mem_menu_pe.exe -noanalysis
 *     -scriptPath research/tools -postScript CampaignStringsProbe.java <output.txt> <pattern>...
 */
public class CampaignStringsProbe extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) {
            printerr("Usage: CampaignStringsProbe.java <output.txt> <pattern>...");
            return;
        }
        Memory mem = currentProgram.getMemory();
        AddressSetView loaded = mem.getLoadedAndInitializedAddressSet();
        try (PrintWriter w = new PrintWriter(new FileWriter(args[0]))) {
            for (int i = 1; i < args.length; i++) {
                String pat = args[i];
                byte[] needle = pat.getBytes("ASCII");
                w.println("### " + pat);
                Address cur = loaded.getMinAddress();
                int hits = 0;
                while (hits < 200) {
                    Address hit = mem.findBytes(cur, needle, null, true, monitor);
                    if (hit == null) {
                        break;
                    }
                    // Walk backwards from the hit over printable bytes to the
                    // start of the run containing it.
                    Address start = hit;
                    while (start.compareTo(loaded.getMinAddress()) > 0) {
                        byte b = mem.getByte(start.subtract(1));
                        if (b < 0x20 || b > 0x7e) {
                            break;
                        }
                        start = start.subtract(1);
                    }
                    StringBuilder sb = new StringBuilder();
                    Address p = start;
                    int len = 0;
                    while (len < 400) {
                        byte b = mem.getByte(p);
                        if (b < 0x20 || b > 0x7e) {
                            break;
                        }
                        sb.append((char) b);
                        p = p.add(1);
                        len++;
                    }
                    w.println(hit + " | " + start + " | " + sb);
                    hits++;
                    cur = p;
                }
                w.println("### " + pat + " total<=" + hits);
                println("PROBE_DONE " + pat + " hits=" + hits);
            }
        }
    }
}
