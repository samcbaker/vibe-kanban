import { ReactNode, useRef } from 'react';
import { PortalContainerContext } from '@/contexts/PortalContainerContext';
import {
  WorkspaceProvider,
  useWorkspaceContext,
} from '@/contexts/WorkspaceContext';
import { ActionsProvider } from '@/contexts/ActionsContext';
import { SequenceTrackerProvider } from '@/keyboard/SequenceTracker';
import { SequenceIndicator } from '@/keyboard/SequenceIndicator';
import { useWorkspaceShortcuts } from '@/keyboard/useWorkspaceShortcuts';
import { ExecutionProcessesProvider } from '@/contexts/ExecutionProcessesContext';
import { LogsPanelProvider } from '@/contexts/LogsPanelContext';
import NiceModal from '@ebay/nice-modal-react';
import { useKeyShowHelp, Scope } from '@/keyboard';
import { KeyboardShortcutsDialog } from '@/components/ui-new/dialogs/KeyboardShortcutsDialog';
import '@/styles/new/index.css';

interface NewDesignScopeProps {
  children: ReactNode;
}

// Wrapper component to get workspaceId from context for ExecutionProcessesProvider
function ExecutionProcessesProviderWrapper({
  children,
}: {
  children: ReactNode;
}) {
  const { workspaceId, selectedSessionId } = useWorkspaceContext();
  return (
    <ExecutionProcessesProvider
      attemptId={workspaceId}
      sessionId={selectedSessionId}
    >
      {children}
    </ExecutionProcessesProvider>
  );
}

function KeyboardShortcutsHandler() {
  useKeyShowHelp(
    () => {
      KeyboardShortcutsDialog.show();
    },
    { scope: Scope.GLOBAL }
  );
  useWorkspaceShortcuts();
  return null;
}

export function NewDesignScope({ children }: NewDesignScopeProps) {
  const ref = useRef<HTMLDivElement>(null);

  return (
    <div ref={ref} className="new-design h-full">
      <PortalContainerContext.Provider value={ref}>
        <WorkspaceProvider>
          <ExecutionProcessesProviderWrapper>
            <LogsPanelProvider>
              <ActionsProvider>
                <SequenceTrackerProvider>
                  <SequenceIndicator />
                  <NiceModal.Provider>
                    <KeyboardShortcutsHandler />
                    {children}
                  </NiceModal.Provider>
                </SequenceTrackerProvider>
              </ActionsProvider>
            </LogsPanelProvider>
          </ExecutionProcessesProviderWrapper>
        </WorkspaceProvider>
      </PortalContainerContext.Provider>
    </div>
  );
}
