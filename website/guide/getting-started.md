# Your first automation

Ten minutes, no account, no OBS. We will make a button on the dashboard do
something, then make something on the internet do something.

The vocabulary you need up front:

- A **trigger** is a thing that happens — a button click, a chat message, a song
  change.
- A **sequence** is what to do about it — a tree of nodes run in order.
- A **profile** owns the pairing: it binds triggers to sequences, and it can be
  switched on and off as a whole.

## 1. Put a button on the dashboard

Open the app. You land on the **Dashboard** — an empty grid.

1. Click **Edit** in the top right. The grid becomes editable.
2. Click **Add widget**. A searchable palette opens.
3. Pick **Button**. It drops onto the grid.
4. Click the widget's ⚙ to set its **Label** to `Hello`, and pick a colour.
5. Drag the bottom-right handle to resize it; drag the widget itself to move it.

<Screenshot src="/screens/dashboard-edit.png" caption="The dashboard in Edit mode, with the widget palette open" />

Leave Edit mode. Clicking the button now does nothing — but it *is* already
firing an event.

::: info What just happened
Clicking a button widget emits the built-in trigger `button.clicked` on the host
event bus, carrying `{ id, label }` where `id` is the widget's instance id. No
WebAssembly is involved — the host publishes it directly. Nothing is listening
yet, which is the next step.
:::

## 2. Bind it to a sequence

Still in Edit mode, the button widget has a **Link an action** tool. Click it.

Quicktys creates (or opens) a profile named `button-<id>` with the
`button.clicked` trigger already bound and already filtered to *this* button
(`id == "<id>"`), and opens it. You can do the same by hand: **Profiles → New**,
then **Add trigger**, pick **Buttons → Button clicked** in the picker, and write
the filter yourself.

<Screenshot src="/screens/profile.png" caption="A profile: an activation condition and one trigger binding" />

The binding has an empty sequence. Add a node:

1. Click **+** to open the node palette.
2. Choose **Action**.
3. In the node, pick the plugin **Notify** and the action **Show toast**.
4. Type `Hello from Quicktys` in the **Message** field.

Leave the profile. Click your button. A toast appears.

<Screenshot src="/screens/getting-started-final.png" caption="The toast fired by the button" />

That is the whole loop: **click → `emit_trigger` → the event bus → the dispatcher
→ your sequence runs.** Every trigger in Quicktys works exactly this way,
including the ones that come from plugins.

## 3. Make it do more than one thing

Go back into the sequence and add a second node — but not another action.

1. Add a **Wait** node. Set its mode to **delay** and its value to `1000`.
2. Add another **Action** node: **Notify → Log text**, message `Still here`.

Run it again. The toast appears, a second passes, and a line lands in the log.

Now try a branch. Add an **If** node between them:

- **Condition**: `true`
- Inside **then**, move your log action.

The condition field is a [Rhai](https://rhai.rs) expression, not a template. It
autocompletes over the reactive state of every loaded plugin — type `@` and see.
`true` is a placeholder; `obs.scene == "Intro"` or `twitch.chat-channel != ""`
are the real thing.

::: warning `=` is not `==`
In Rhai `=` assigns and `==` compares. A condition that fails to compile is
treated as **false**, silently — so a typo disables the binding with no error
message. If a binding mysteriously never runs, check the condition first.
:::

## 4. React to something real

Buttons are a good first loop but a poor demo. Let us watch a Twitch chat — which
needs **no account at all**, because Quicktys logs in anonymously for read-only
chat.

1. **Marketplace** → install the **Twitch** plugin. It pulls the certified
   `twitch` module as a dependency and shows you a consent screen for the
   capabilities that module asks for (`net`).
2. In any sequence, run the action **Twitch → Watch a channel's chat** once, with
   the channel name. (A button is a fine way to trigger it.)
3. Create a new profile with the trigger **Twitch → Chat message**.
4. Give it an action: **Notify → Show toast**, message:

   ```
   @twitch.chat-channel: {{ message }}
   ```

Every message in that channel now raises a toast. Two syntaxes appear there and
they are not the same thing:

- `@twitch.chat-channel` reads a **reactive state variable** published by the
  plugin.
- `{{ message }}` reads a field of the **trigger context** — the payload this
  particular occurrence carried.

Both are covered in [Variables & templating](/guide/variables).

::: tip Filter before you act
A trigger binding has a **filter** — a Rhai expression evaluated against the
context. `message.starts_with("!")` turns "every chat message" into "only
commands". Filtering in the binding is cheaper and clearer than an `if` at the
top of the sequence.
:::

## 5. Put it on stream

Chat as toasts is for you, not your viewers. To put it on the stream:

1. Install the **Overlays** plugin (it pulls the `webview` module, which is what
   actually stores overlays and ships the editor).
2. Open the **Overlays** tab, create one, and drop the **Twitch chat** widget on
   the canvas. It renders for real, live, while you position it.
3. In OBS, add a **Browser source** pointing at
   `http://localhost:8787/overlay/<id>`.

See [Overlays](/guide/overlays) for the full tour.

## Where to go next

- [Profiles & triggers](/guide/profiles) — activation conditions, filters, and
  why profiles exist at all.
- [Automations](/guide/automations) — every node type, queues, and structured
  concurrency.
- [Variables & templating](/guide/variables) — `@`, `{{ }}`, and Rhai.
- [Integrations](/guide/integrations) — what each shipped plugin can do.
