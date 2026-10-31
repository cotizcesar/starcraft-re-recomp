# original/ — instalación de referencia (intacta)

No se copian los archivos del juego: se trabaja siempre en modo lectura sobre la
instalación y todo resultado se escribe en `~/ReverseEngineering/`.

| Campo | Valor |
|---|---|
| Ruta | `/var/home/cotizcesar/Games/battlenet/drive_c/Program Files (x86)/StarCraft` |
| Origen | Battle.net (Lutris, prefix de Wine) |
| Versión | 1.23.10.13515 (`.build.info`, Build Key `864772b9ff94f6d372aa4ee90ee2f8ab`) |
| Archivos | 724 |
| Tamaño | 5 879 150 935 B (5,5 GiB) |
| Verificación | `sha256sum -c ~/ReverseEngineering/manifests/sha256.txt` (desde la raíz del juego) |

Reglas:
1. Nunca escribir, renombrar ni borrar dentro de esta ruta.
2. Nunca ejecutar un binario desconocido; el juego se lanza solo vía Lutris.
3. Cualquier extracción se escribe en `extracted/` conservando el original.
