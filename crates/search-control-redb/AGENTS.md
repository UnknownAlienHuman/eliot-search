# Working on root control

Preserve plain_control_meta, vendor redb 4.5.0, root initialization, read-only inspection, savepoint checks, empty-probe bounds, and incomplete-root discard behavior.

Current source namespace, locator index, and source rows live in the same control/control.redb file. Capture is read-only before file I/O. The short source-row transaction follows verified object readback and atomically compares the namespace, source ID, row revision, registration revision, and locator binding. New current tables are ordinary Current-root data; do not call the unfinished-root empty probe during source operations or introduce a second database.
