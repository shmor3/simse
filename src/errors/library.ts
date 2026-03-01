// Re-export library errors from simse-vector for backwards compatibility
export {
	createEmbeddingError,
	createLibraryError,
	createStacksCorruptionError,
	createStacksError,
	createStacksIOError,
	isEmbeddingError,
	isLibraryError,
	isStacksCorruptionError,
	isStacksError,
	isStacksIOError,
} from '../ai/library/errors.js';
