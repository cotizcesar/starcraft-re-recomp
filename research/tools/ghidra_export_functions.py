# -*- coding: utf-8 -*-
"""Exporta el inventario de funciones del programa actual a CSV.

Uso (headless):
  analyzeHeadless <projdir> <proj> -process <exe> -noanalysis \
    -scriptPath ~/ReverseEngineering/tools -postScript ghidra_export_functions.py <salida.csv>
"""
import csv

args = getScriptArgs()
out_path = args[0]
fm = currentProgram.getFunctionManager()
n = 0
with open(out_path, "w", newline="") as f:
    w = csv.writer(f)
    w.writerow(["entry", "name", "size", "is_thunk", "is_external"])
    for fn in fm.getFunctions(True):
        try:
            w.writerow([
                "0x%x" % fn.getEntryPoint().getOffset(),
                fn.getName(),
                fn.getBody().getNumAddresses(),
                bool(fn.isThunk()),
                bool(fn.isExternal()),
            ])
            n += 1
        except Exception:
            pass
print("FUNC_EXPORT_DONE functions=%d program=%s" % (n, currentProgram.getName()))
