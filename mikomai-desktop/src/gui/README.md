# GUI architecture

The frontend is mounted as `Root` → `Mediator` → visual components.

- `Root` owns provider composition and mounts only the mediator.
- `Mediator` renders the root workspace mediator and its state machine.
- `GuiEventScope` implements the Chain of Responsibility. A child handler may
  consume an event; otherwise it bubbles to its parent and finally to the
  root mediator.
- Components expose passive View functions that receive display state and
  callbacks from their Presenter. IPC, subscriptions, and state transitions
  remain in presenters or the mediator.
