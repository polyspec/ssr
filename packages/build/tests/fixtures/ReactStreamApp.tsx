import React, { Suspense } from "react";

class Boundary extends React.Component<{ children: React.ReactNode }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  render() {
    return this.state.failed ? <p id="client-error">Client error</p> : this.props.children;
  }
}

let attemptedOnServer = false;
let rejectOnServer!: (reason: Error) => void;
const suspendedOnServer = new Promise<void>((_resolve, reject) => {
  rejectOnServer = reject;
});

function Deferred({ failClient }: { failClient: boolean }) {
  if (typeof window === "undefined") {
    if (!attemptedOnServer) {
      attemptedOnServer = true;
      Promise.resolve().then(() => rejectOnServer(new Error("late server failure")));
      throw suspendedOnServer;
    }
    throw new Error("late server failure");
  }
  if (failClient) {
    throw new Error("client retry failure");
  }
  return <p id="recovered">Recovered</p>;
}

export default function App({ failClient, failShell, hangShell, renderState }: {
  failClient: boolean;
  failShell?: boolean;
  hangShell?: boolean;
  renderState: { input: unknown; output: unknown };
}) {
  if (hangShell) {
    while (true) {}
  }
  if (failShell) {
    throw new Error("before shell failure");
  }
  renderState.output = renderState.input;
  return (
    <Boundary>
      <main>
        <h1>Ready</h1>
        <Suspense fallback={<p id="fallback">Loading</p>}>
          <Deferred failClient={failClient} />
        </Suspense>
      </main>
    </Boundary>
  );
}
