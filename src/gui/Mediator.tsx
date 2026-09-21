import { RootMediator } from "../components/AppLayout/AppLayout";

/**
 * The application mediator owns the UI state machine and receives events from
 * the bubbling GUI event chain. It is intentionally the only child mounted by
 * Root; every visual component is rendered below this boundary.
 */
export function Mediator() {
  return <RootMediator />;
}
