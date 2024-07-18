local badge_join = {
  from: 'badges',
  on: [u.eq('badges.employee', 'employees.id')],
};
{
  select: {
    fields: [
      'employees.*',
      u.as('badges.id', 'badge'),
    ],
    from: 'employees',
    joins+: [badge_join],
    where: 'employed',
  } + u.where_and([u.ge('age', 30)]),
}
