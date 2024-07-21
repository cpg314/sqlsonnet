local var1 = 0;
local var2 = 'test';
{
  select: {
    fields: [
      u.as(1 + 1, 'two'),
      // Grafana variables expansion
      ${var1},
      // ${var2} would produce the identifier `test`
      // '${var2}' would produce the string 'test'
      u.as(${var2:singlequote}, "a_string"),
    ],
  }
}
