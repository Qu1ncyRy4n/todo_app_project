# Task Field and Quick-Entry Survey

Date: 2026-10-14
Feeds: `vazut-pigat` (review other todo apps), `popal-nofah` (keyword entry),
`farag-susot` (typed path syntax).

## Obsidian Tasks

Source: <https://publish.obsidian.md/tasks/Reference/Task+Formats/Tasks+Emoji+Format>,
<https://publish.obsidian.md/tasks/Reference/Task+Formats/Dataview+Format>.

Two interchangeable inline formats on a Markdown checkbox line: emoji
signifiers, or Dataview `[key:: value]` fields (parentheses also accepted;
Tasks rewrites them with brackets).

| Field | Emoji | Dataview key |
|---|---|---|
| created | ➕ | `created` |
| scheduled | ⏳ | `scheduled` |
| start | 🛫 | `start` |
| due | 📅 | `due` |
| done | ✅ | `completion` |
| cancelled | ❌ | `cancelled` |
| priority | 🔺 highest, ⏫ high, 🔼 medium, 🔽 low, ⏬ lowest; none = normal | `priority` (`lowest`..`highest`) |
| recurrence | 🔁 `every day when done` | `repeat` |
| id | 🆔 | `id` |
| depends on | ⛔ `id1,id2` | `dependsOn` |
| on completion | 🏁 `keep` / `delete` | `onCompletion` |

Tags are ordinary Obsidian `#tag/nested` hashtags. Values are dates as
`YYYY-MM-DD`.

## Super Productivity

Source: <https://super-productivity.com/blog/getting-started-with-super-productivity/>,
<https://github.com/johannesjo/super-productivity>, search summaries.

Short syntax parsed from the title on entry (can be disabled in settings):

| Syntax | Meaning |
|---|---|
| `#tag` | tag |
| `+project` | project, prefix match, minimum 3 characters |
| `@friday`, `@tomorrow 3pm` | due date, optionally with time |
| `30m` | time estimate |
| `1h/2h` | time spent / estimate |

Confirmed fields: title, notes, subtasks, tags, project, time estimate, time
spent, due date and time. Priority is expressed through tags. Reminders,
repeat, attachments, and issue-tracker integration exist as advanced features
but were not confirmed from the pages read.

## Comparison with this ledger

| Field | Ledger today | Obsidian | Super Productivity |
|---|---|---|---|
| due | `todo due` | 📅 | `@date` |
| defer / start | `todo defer` | 🛫 start | — |
| scheduled date | absent | ⏳ | due date+time |
| time of day | absent (handoff: deliberate absence) | — | `@tomorrow 3pm` |
| priority | 4 levels | 6 levels | via tags |
| estimate | `todo estimate` minutes | — | `30m` |
| time spent | `todo session` | — | `1h/2h` |
| difficulty | 1-5 | — | — |
| depends | `todo depends` | ⛔ | — |
| recurrence | absent | 🔁 | repeat (advanced) |
| created/done/cancelled | derived from events | ➕ ✅ ❌ written inline | — |
| typed relations | `todo relate` | — | — |

Gaps worth noting: recurrence and a scheduled date. Created and done dates
need no field because events already record them.

Recorded by agent-assistant.
