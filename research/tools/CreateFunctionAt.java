import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;

/**
 * Creates a function (minimal body) at each given address, only if there is not
 * already one containing it. This is analysis progress on OUR database;
 * it does not touch the original. Usage:
 *   ... -postScript CreateFunctionAt.java <addr1> ...
 */
public class CreateFunctionAt extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        FunctionManager fm = currentProgram.getFunctionManager();
        for (String a : args) {
            Address addr = currentProgram.getAddressFactory()
                    .getDefaultAddressSpace().getAddress(a);
            if (fm.getFunctionContaining(addr) != null) {
                println("HASFUNC " + a);
                continue;
            }
            try {
                Function fn = createFunction(addr, null);
                println("CREATED " + a + " -> "
                        + (fn == null ? "null" : fn.getName()));
            }
            catch (Exception e) {
                println("CREATEFAIL " + a + " " + e.getMessage());
            }
        }
        println("CREATE_BATCH_DONE");
    }
}
