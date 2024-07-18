// SELECT 3 * (1 + 2)
{ select: { fields: [u.op('*', [3, u.op('+', [1, 2])])] } }
