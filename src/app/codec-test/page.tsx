import { BackendProvider } from '@/lib/backend/BackendContext';
import CodecTestContent from './components/CodecTestContent';

export default function CodecTestPage() {
  return (
    <BackendProvider>
      <CodecTestContent />
    </BackendProvider>
  );
}
