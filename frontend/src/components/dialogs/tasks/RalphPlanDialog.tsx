import { useState, useEffect } from 'react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Alert } from '@/components/ui/alert';
import { ralphApi } from '@/lib/api';
import type { TaskWithAttemptStatus, RalphStatus } from 'shared/types';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/lib/modals';
import { useQueryClient } from '@tanstack/react-query';
import { Loader2, CheckCircle2, XCircle, RefreshCw } from 'lucide-react';

export interface RalphPlanDialogProps {
  task: TaskWithAttemptStatus;
}

type DialogMode = 'approval' | 'readonly' | 'error';

function getDialogMode(status: RalphStatus): DialogMode {
  switch (status) {
    case 'awaitingapproval':
      return 'approval';
    case 'completed':
      return 'readonly';
    case 'failed':
      return 'error';
    default:
      return 'readonly';
  }
}

const RalphPlanDialogImpl = NiceModal.create<RalphPlanDialogProps>(
  ({ task }) => {
    const modal = useModal();
    const queryClient = useQueryClient();
    const [planContent, setPlanContent] = useState<string | null>(null);
    const [isLoading, setIsLoading] = useState(true);
    const [isApproving, setIsApproving] = useState(false);
    const [isReplanning, setIsReplanning] = useState(false);
    const [isRestarting, setIsRestarting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const mode = getDialogMode(task.ralph_status);

    useEffect(() => {
      async function fetchPlan() {
        setIsLoading(true);
        setError(null);
        try {
          const plan = await ralphApi.getPlan(task.id);
          setPlanContent(plan);
        } catch (err) {
          const errorMessage =
            err instanceof Error ? err.message : 'Failed to load plan';
          setError(errorMessage);
        } finally {
          setIsLoading(false);
        }
      }
      fetchPlan();
    }, [task.id]);

    const handleApprove = async () => {
      setIsApproving(true);
      setError(null);
      try {
        await ralphApi.approvePlan(task.id);
        queryClient.invalidateQueries({ queryKey: ['tasks'] });
        modal.resolve('approved');
        modal.hide();
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : 'Failed to approve plan';
        setError(errorMessage);
      } finally {
        setIsApproving(false);
      }
    };

    const handleReplan = async () => {
      setIsReplanning(true);
      setError(null);
      try {
        await ralphApi.rerunPlan(task.id);
        queryClient.invalidateQueries({ queryKey: ['tasks'] });
        modal.resolve('replanned');
        modal.hide();
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : 'Failed to replan';
        setError(errorMessage);
      } finally {
        setIsReplanning(false);
      }
    };

    const handleRestart = async () => {
      setIsRestarting(true);
      setError(null);
      try {
        await ralphApi.restart(task.id);
        queryClient.invalidateQueries({ queryKey: ['tasks'] });
        modal.resolve('restarted');
        modal.hide();
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : 'Failed to restart';
        setError(errorMessage);
      } finally {
        setIsRestarting(false);
      }
    };

    const handleClose = () => {
      modal.reject();
      modal.hide();
    };

    const isProcessing = isApproving || isReplanning || isRestarting;

    const getTitle = () => {
      switch (mode) {
        case 'approval':
          return 'Review Implementation Plan';
        case 'readonly':
          return 'Implementation Plan (Completed)';
        case 'error':
          return 'Ralph Execution Failed';
        default:
          return 'Implementation Plan';
      }
    };

    const getDescription = () => {
      switch (mode) {
        case 'approval':
          return 'Review the plan below and approve to start building, or request a replan.';
        case 'readonly':
          return 'This plan has been completed successfully.';
        case 'error':
          return 'The Ralph execution encountered an error. You can restart or view the plan.';
        default:
          return '';
      }
    };

    return (
      <Dialog open={modal.visible} onOpenChange={(open) => !open && handleClose()}>
        <DialogContent className="max-w-3xl max-h-[80vh] flex flex-col">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              {mode === 'approval' && (
                <RefreshCw className="h-5 w-5 text-orange-500" />
              )}
              {mode === 'readonly' && (
                <CheckCircle2 className="h-5 w-5 text-green-500" />
              )}
              {mode === 'error' && (
                <XCircle className="h-5 w-5 text-red-500" />
              )}
              {getTitle()}
            </DialogTitle>
            <DialogDescription>{getDescription()}</DialogDescription>
          </DialogHeader>

          <div className="flex-1 min-h-0">
            {isLoading ? (
              <div className="flex items-center justify-center py-8">
                <Loader2 className="h-6 w-6 animate-spin" />
                <span className="ml-2">Loading plan...</span>
              </div>
            ) : error && !planContent ? (
              <Alert variant="destructive">{error}</Alert>
            ) : planContent ? (
              <div className="h-[400px] border rounded-md p-4 overflow-auto">
                <pre className="whitespace-pre-wrap font-mono text-sm">
                  {planContent}
                </pre>
              </div>
            ) : (
              <Alert variant="default">No plan content available.</Alert>
            )}
          </div>

          {error && planContent && (
            <Alert variant="destructive" className="mt-2">
              {error}
            </Alert>
          )}

          <DialogFooter className="gap-2 sm:gap-0">
            {mode === 'approval' && (
              <>
                <Button
                  variant="outline"
                  onClick={handleReplan}
                  disabled={isProcessing || isLoading}
                >
                  {isReplanning ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      Replanning...
                    </>
                  ) : (
                    'Replan'
                  )}
                </Button>
                <Button
                  onClick={handleApprove}
                  disabled={isProcessing || isLoading}
                >
                  {isApproving ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      Approving...
                    </>
                  ) : (
                    'Approve & Build'
                  )}
                </Button>
              </>
            )}

            {mode === 'error' && (
              <>
                <Button
                  variant="outline"
                  onClick={handleClose}
                  disabled={isProcessing}
                >
                  Close
                </Button>
                <Button
                  onClick={handleRestart}
                  disabled={isProcessing || isLoading}
                >
                  {isRestarting ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      Restarting...
                    </>
                  ) : (
                    'Restart Ralph'
                  )}
                </Button>
              </>
            )}

            {mode === 'readonly' && (
              <Button variant="outline" onClick={handleClose}>
                Close
              </Button>
            )}
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const RalphPlanDialog = defineModal<
  RalphPlanDialogProps,
  'approved' | 'replanned' | 'restarted'
>(RalphPlanDialogImpl);
