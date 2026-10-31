import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;

/**
 * Crea una funcion (body minimo) en cada direccion dada, solo si no hay ya
 * una que la contenga. Es progreso de analisis sobre NUESTRA base de datos;
 * no toca el original. Uso:
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
