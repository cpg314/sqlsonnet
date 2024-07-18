// Utilities embedded in sqlsonnet. Use with
// local u = import 'utils.libsonnet';
{
  op(operator, l, empty=null): if std.length(l) == 0 then empty else if std.length(l) == 1 then l[0] else [l[0], operator, self.op(operator, l[1:], empty)],
  select(x): { select: x },
  where_and(expr): { where: $.and([expr] + (if 'where' in super then [super.where] else [])) },
  having_and(expr): { having: $.and([expr] + (if 'having' in super then [super.having] else [])) },
  // Operators
  and(l): self.op('AND', l, true),
  sum(l): self.op('+', l, 0),
  prod(l): self.op('*', l, 1),
  div(a, b): self.op('/', [a, b]),
  sub(a, b): self.op('-', [a, b]),
  or(l): self.op('OR', l),
  string(s): "'" + s + "'",
  eq(a, b): [a, '=', b],
  ge(a, b): [a, '>=', b],
  le(a, b): [a, '<=', b],
  gt(a, b): [a, '>', b],
  lt(a, b): [a, '<', b],
  leq(a, b): [a, '<=', b],
  in_(a, b): [a, 'IN', b],
  // expr AS as, overriding existing aliases.
  as(expr, as): if std.isObject(expr) && std.objectHas(expr, 'alias') then { expr: expr.expr, alias: as } else { expr: expr, alias: as },
  // Functions
  fn(name, params): { fn: name, params: params },
  count(expr='*', as='c'): self.as(self.fn('count', [expr]), as),
  rand(): self.fn('rand', []),
}
