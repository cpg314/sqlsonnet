// Utilities embedded in sqlsonnet. Use with
// local u = import 'utils.libsonnet';
{
  // Operators
  and(l): std.join(' AND ', l),
  or(l): std.join(' OR ', l),
  eq(a, b): std.join(' ', [a, '=', b]),
  ge(a, b): std.join(' ', [a, '>', b]),
  le(a, b): std.join(' ', [a, '<', b]),
  geq(a, b): std.join(' ', [a, '>=', b]),
  leq(a, b): std.join(' ', [a, '<=', b]),
  in_(a, b): std.join(' ', [a, 'IN', b]),
  // expr AS as
  as(expr, as): { expr: expr, alias: as },
  // Functions
  count(expr='*', as='c'): self.as(std.format('count(%s)', expr), as),
}
