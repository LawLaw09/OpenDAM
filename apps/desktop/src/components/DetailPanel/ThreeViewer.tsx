import { Canvas } from '@react-three/fiber';
import { useGLTF, Stage, PresentationControls } from '@react-three/drei';
import { Suspense, Component, type ReactNode } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';

interface ErrorBoundaryProps {
  fallback: ReactNode;
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
}

class ThreeErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  componentDidCatch(error: any, errorInfo: any) {
    console.error('ThreeViewer render error:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return this.props.fallback;
    }
    return this.props.children;
  }
}

function Model({ url }: { url: string }) {
  const fileUrl = convertFileSrc(url);
  const { scene } = useGLTF(fileUrl);
  return <primitive object={scene} />;
}

export function ThreeViewer({ modelPath }: { modelPath: string }) {
  return (
    <ThreeErrorBoundary
      fallback={
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', color: 'var(--color-text-muted)', fontSize: 13 }}>
          Unable to render 3D preview
        </div>
      }
    >
      <Canvas shadows dpr={[1, 2]} camera={{ fov: 50 }}>
        <Suspense fallback={null}>
          <PresentationControls speed={1.5} global zoom={0.5} polar={[-0.1, Math.PI / 4]}>
            <Stage environment="city" intensity={0.5}>
              <Model url={modelPath} />
            </Stage>
          </PresentationControls>
        </Suspense>
      </Canvas>
    </ThreeErrorBoundary>
  );
}

