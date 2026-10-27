import { ref } from 'vue';

export const confirmState = ref({
  isOpen: false,
  title: 'Confirm Action',
  message: '',
  confirmText: 'Confirm',
  cancelText: 'Cancel',
  isDanger: true,
  resolve: null,
});

/**
 * Modern non-blocking, glassmorphic replacement for window.confirm()
 * @param {Object} options
 * @param {string} options.title
 * @param {string} options.message
 * @param {string} [options.confirmText='Confirm']
 * @param {string} [options.cancelText='Cancel']
 * @param {boolean} [options.isDanger=true]
 * @returns {Promise<boolean>}
 */
export function askConfirm({
  title = 'Confirm Action',
  message = 'Are you sure you want to proceed?',
  confirmText = 'Confirm',
  cancelText = 'Cancel',
  isDanger = true,
} = {}) {
  return new Promise((resolve) => {
    confirmState.value = {
      isOpen: true,
      title,
      message,
      confirmText,
      cancelText,
      isDanger,
      resolve,
    };
  });
}

export function handleConfirmResponse(choice) {
  if (confirmState.value.resolve) {
    confirmState.value.resolve(choice);
  }
  confirmState.value.isOpen = false;
  confirmState.value.resolve = null;
}
