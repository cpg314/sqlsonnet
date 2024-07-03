// Utilities embedded in sqlsonnet. Use with
// local u = import 'utils.libsonnet';
{

  op(operator, l): if std.length(l) == 1 then l[0] else [l[0], operator, self.and(l[1:])],
  select(x): { select: x },
  // Operators
  and(l): self.op('AND', l),
  or(l): self.op('OR', l),
  string(s): "'" + s + "'",
  eq(a, b): [a, '=', b],
  ge(a, b): [a, '>=', b],
  le(a, b): [a, '<=', b],
  gt(a, b): [a, '>', b],
  lt(a, b): [a, '<', b],
  leq(a, b): [a, '<=', b],
  in_(a, b): [a, 'IN', b],
  // expr AS as
  as(expr, as): { expr: expr, alias: as },
  // Functions
  fn(name, params): { fn: name, params: params },
  count(expr='*', as='c'): self.as(self.fn('count', [expr]), as),
}
