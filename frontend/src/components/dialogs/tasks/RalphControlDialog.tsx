import { useState, useCallback } from 'react';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/lib/modals';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Alert } from '@/components/ui/alert';
import { ralphApi } from '@/lib/api';
import type { Task } from 'shared/types';

interface RalphControlDialogProps {
  task: Task;
}

const RalphControlDialogImpl = NiceModal.create<RalphControlDialogProps>(
  ({ task }) => {
    const modal = useModal();
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const handleAction = useCallback(
      async (
        action: () => Promise<{ success: boolean; message: string }>,
        successMessage: string
      ) => {
        setLoading(true);
        setError(null);

        try {
          console.log(`[Ralph] Executing action for task ${task.id}`);
          const response = await action();

          if (response.success) {
            console.log(`[Ralph] Success: ${successMessage}`);
            modal.remove();
          } else {
            console.error(`[Ralph] Failed: ${response.message}`);
            setError(response.message);
          }
        } catch (err) {
          const message = err instanceof Error ? err.message : 'Unknown error';
          console.error(`[Ralph] Error:`, err);
          setError(message);
        } finally {
          setLoading(false);
        }
      },
      [task.id, modal]
    );

    const startPlan = useCallback(
      () =>
        handleAction(
          () => ralphApi.startPlan(task.id),
          'Plan mode started - check Terminal'
        ),
      [handleAction, task.id]
    );

    const startBuild = useCallback(
      () =>
        handleAction(
          () => ralphApi.startBuild(task.id),
          'Build mode started - check Terminal'
        ),
      [handleAction, task.id]
    );

    const stop = useCallback(
      () =>
        handleAction(() => ralphApi.stop(task.id), 'Stop signal sent'),
      [handleAction, task.id]
    );

    const openPlan = useCallback(async () => {
      try {
        console.log('[Ralph] Opening IMPLEMENTATION_PLAN.md');
        const response = await ralphApi.openPlan(task.id);
        if (!response.success) {
          setError(response.message);
        }
      } catch (err) {
        const message =
          err instanceof Error ? err.message : 'Failed to open plan';
        console.error('[Ralph] Failed to open plan:', err);
        setError(message);
      }
    }, [task.id]);

    const openTerminal = useCallback(async () => {
      try {
        console.log('[Ralph] Opening Terminal');
        await ralphApi.openTerminal(task.id);
      } catch (err) {
        console.error('[Ralph] Failed to open Terminal:', err);
        setError('Failed to open Terminal');
      }
    }, [task.id]);

    return (
      <Dialog open={modal.visible} onOpenChange={(open) => !open && modal.remove()}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Ralph Mode</DialogTitle>
            <DialogDescription className="text-left">
              {task.title}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <p className="text-sm text-muted-foreground">
              Ralph will open in a Terminal window. Plan mode analyzes and
              creates an implementation plan. Build mode executes the plan.
            </p>

            {error && (
              <Alert variant="destructive">
                <strong>Error:</strong> {error}
              </Alert>
            )}

            <div className="flex flex-col gap-2">
              <Button
                onClick={startPlan}
                disabled={loading}
                className="w-full justify-start"
              >
                Start Plan (max 10 iterations)
              </Button>
              <Button
                onClick={startBuild}
                disabled={loading}
                className="w-full justify-start"
              >
                Start Build (max 20 iterations)
              </Button>
              <Button
                variant="secondary"
                onClick={openPlan}
                disabled={loading}
                className="w-full justify-start"
              >
                Open Plan
              </Button>
              <Button
                variant="secondary"
                onClick={openTerminal}
                disabled={loading}
                className="w-full justify-start"
              >
                Open Terminal
              </Button>
              <Button
                variant="destructive"
                onClick={stop}
                disabled={loading}
                className="w-full justify-start"
              >
                Stop Ralph
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    );
  }
);

export const RalphControlDialog = defineModal<RalphControlDialogProps, void>(
  RalphControlDialogImpl
);
