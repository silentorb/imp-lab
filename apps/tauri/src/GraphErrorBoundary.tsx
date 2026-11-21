import { Component, type ErrorInfo, type ReactNode } from "react";
import { Button, Stack, Text } from "@mantine/core";

type Props = {
  children: ReactNode;
  resetKey?: string;
};

type State = {
  error: Error | null;
};

/** Keeps the Lab shell visible if the graph canvas throws (e.g. bad document). */
export class GraphErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error("Imp graph view crashed", error, info.componentStack);
  }

  componentDidUpdate(prevProps: Props): void {
    if (prevProps.resetKey !== this.props.resetKey && this.state.error) {
      this.setState({ error: null });
    }
  }

  render(): ReactNode {
    if (this.state.error) {
      return (
        <Stack gap="sm" p="md" align="flex-start">
          <Text size="sm" c="red">
            Graph view failed to render: {this.state.error.message}
          </Text>
          <Button
            size="xs"
            variant="default"
            onClick={() => this.setState({ error: null })}
          >
            Retry
          </Button>
        </Stack>
      );
    }
    return this.props.children;
  }
}
