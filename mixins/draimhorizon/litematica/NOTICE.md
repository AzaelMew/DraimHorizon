# Litematica ports (LGPL-3.0)

The files in this directory are **not** MIT. They are Rust ports of parts of
[Litematica](https://github.com/sakura-ryoko/litematica) (a fork of maruohon's Litematica, 1.21.x
branch, commit `eb15b39`) by maruohon, sakura-ryoko and the Litematica contributors, licensed
under the **GNU Lesser General Public License v3.0** (`LICENSE`, unchanged). Each file names the
Java sources it follows. The rest of DraimHorizon stays under the MIT license and uses this
directory only through its public functions.

- `verifier.rs`: `SchematicVerifier` (`checkBlockStates`, the mismatch types, the overview and
  the sort criteria, `addAndSortPositions`).
- `materials.rs`: `MaterialListEntry`, `MaterialListSorter` and the counting of `MaterialListUtils`.
- `selection.rs`: `Box`, `AreaSelection` and `PositionUtils` corner handling.
- `writer.rs`: `LitematicaSchematic` (the writing of a region to the file) and
  `LitematicaBitArray`.

Not ported: anything that needs the game's classes (rendering, GUI, tasks, `BlockState` objects):
those are written in DraimHorizon's own code and call into these files with plain data.
