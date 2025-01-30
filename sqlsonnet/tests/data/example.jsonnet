// Example used in README.md
[
  {
    select: {
      // List of expressions
      fields: [
        // Primitive types
        1,
        1.0,
        true,
        '"string"',
        // Column reference
        'col',
        // Aliased expression
        u.as('col', 'alias'),
        // Operator, equivalent to [1, "+", 2]
        u.op('+', [1, 2]),
        // Equivalent to u.op("=", [1, 2])
        u.eq(1, 2),
        // Function, equivalent to {fn: "count", params: ["*"]}
        u.fn('count', ['*']),
        // Negation
        u.not(u.neq(1, 2)),
        // Like operator
        u.like('text', u.string('%t%')),
      ],
      // From expression (optional)
      // Table name
      from: 'a',
      // // Aliased table name
      // from: { table: 'a', as: 'b' },
      // // Subquery with optional alias
      // from: { fields: ['*'], from: 'b', as: 'c' },
      // List of expressions (optional)
      groupBy: [],
      // List of joins (optional)
      joins: [
        // From expression and ON (list of boolean expressions)
        { from: 'b', on: ['f1=f2'] },
        // From expression and USING (list of column identifiers)
        { from: 'c', using: ['f'] },
        // An empty `on` or `using` parameter results in a CROSS JOIN
        { from: 'd', using: [] },
        // Other types of joints
        { from: 'e', using: ['f'], kind: 'left-outer' },
      ],
      // Expression (optional). Use u.and, u.or to combine.
      having: true,
      // Expression (optional). Use u.and, u.or to combine.
      where: true,
      // List of identifiers or { expr: identifier, order: "desc" } or { expr: identifier, order: "asc" }
      orderBy: ['col1', { expr: 'col2', order: 'desc' }, { expr: 'col3', order: 'asc' }],
      // Integer (optional)
      limit: 100,
      // Integer (optional)
      sample: 100,
      // Integer (optional)
      offset: 10,
      // List of expressions (optional)
      settings: ['join_algorithm="parallel_hash"'],
    },
  },
  // Adding fields and JOINs
  u.select(
    {
      fields: [0],
      from: 'a',
      joins: [{ from: 'b', using: ['col1'] }],
    } + {
      fields+: [1],
      joins+: [{ from: 'c', using: ['col2'] }],
    }
  ),
  // Adding WHERE conditions
  u.select(
    {
      fields: [0],
      from: 'a',
      where: u.eq(1, 1),
    } + u.where_and([u.ge(2, 1)]),
  ),
  u.select(
    {
      fields: [0],
      limit: 10,
      limitBy: ['id', u.sub('debits', 'credits')],
    }
  ),
]
