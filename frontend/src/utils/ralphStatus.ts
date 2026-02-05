/**
 * Ralph status configuration and utilities
 *
 * This module provides configuration for displaying Ralph status in the UI,
 * including icons, labels, colors, and animations for each status state.
 */

import { type RalphStatus } from 'shared/types';
import {
  Bot,
  FileCheck,
  Hammer,
  CheckCircle2,
  XCircle,
  type LucideIcon,
} from 'lucide-react';

export interface RalphStatusConfig {
  icon: LucideIcon | null;
  label: string;
  color: string;
  animate?: string;
  clickable: boolean;
  action?: 'openPlanDialog' | 'openErrorDialog' | 'openReadOnlyPlan';
}

/**
 * Configuration for each Ralph status state
 */
export const ralphStatusConfig: Record<RalphStatus, RalphStatusConfig> = {
  none: {
    icon: null,
    label: '',
    color: '',
    clickable: false,
  },
  planning: {
    icon: Bot,
    label: 'Planning...',
    color: 'text-purple-500',
    animate: 'animate-pulse',
    clickable: false,
  },
  awaitingapproval: {
    icon: FileCheck,
    label: 'Review Plan',
    color: 'text-orange-500',
    clickable: true,
    action: 'openPlanDialog',
  },
  building: {
    icon: Hammer,
    label: 'Building...',
    color: 'text-blue-500',
    animate: 'animate-bounce',
    clickable: false,
  },
  completed: {
    icon: CheckCircle2,
    label: 'Complete',
    color: 'text-green-500',
    clickable: true,
    action: 'openReadOnlyPlan',
  },
  failed: {
    icon: XCircle,
    label: 'Failed',
    color: 'text-red-500',
    clickable: true,
    action: 'openErrorDialog',
  },
};

/**
 * Check if a Ralph status is active (not None or Completed)
 */
export function isRalphActive(status: RalphStatus): boolean {
  return (
    status === 'planning' ||
    status === 'awaitingapproval' ||
    status === 'building'
  );
}

/**
 * Check if Ralph can be cancelled from this status
 */
export function canCancelRalph(status: RalphStatus): boolean {
  return (
    status === 'planning' ||
    status === 'awaitingapproval' ||
    status === 'building' ||
    status === 'failed'
  );
}

/**
 * Check if Ralph can be started from this status
 */
export function canStartRalph(status: RalphStatus): boolean {
  return status === 'none' || status === 'failed';
}

/**
 * Check if Ralph can be restarted from this status
 */
export function canRestartRalph(status: RalphStatus): boolean {
  return status === 'failed';
}

/**
 * Check if Ralph can be reset from this status
 */
export function canResetRalph(status: RalphStatus): boolean {
  return status === 'completed';
}

/**
 * Get human-readable status description
 */
export function getRalphStatusDescription(status: RalphStatus): string {
  switch (status) {
    case 'none':
      return 'Ralph is not active';
    case 'planning':
      return 'Ralph is analyzing the task and creating an implementation plan';
    case 'awaitingapproval':
      return 'The implementation plan is ready for review';
    case 'building':
      return 'Ralph is implementing the approved plan';
    case 'completed':
      return 'Ralph has completed the implementation';
    case 'failed':
      return 'Ralph execution failed';
    default:
      return 'Unknown status';
  }
}
