; The same two-byte encoding feeds both execution engines.
SET r0, 250
SET r1, 15
ADD r0, r1
ADD r1, r0
SET r2, 0
ADD r2, r2
HALT
