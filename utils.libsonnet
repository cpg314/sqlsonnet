{
  eq(a, b): [a, '=', b],
  neq(a, b): [a, '!=', b],
  ge(a, b): [a, '>', b],
  le(a, b): [a, '<', b],
  geq(a, b): [a, '>=', b],
  leq(qa, b): [a, '=<', b],
  in_(a, b): [a, 'IN', b],
  and(l): { and: l },
  or(l): { or: l },
  count(expr='*', as='c'): { expr: std.format('count(%s)', expr), as: as },
}
