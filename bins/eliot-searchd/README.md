# eliot-searchd

The daemon. Sole owner of the data root and of the supervised index process.

Owns identity, inventory, revisions, preparation, policy, publication, exact scan, query recipes,
readback and result projection. Every client — CLI, ELIOT, optional workers — reaches storage only
through it.
