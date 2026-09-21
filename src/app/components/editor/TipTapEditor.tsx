'use client';
import React from 'react';
import { useEditor, EditorContent } from '@tiptap/react';
import {
  Bold,
  Italic,
  UnderlineIcon,
  Heading1,
  Heading2,
  Heading3,
  Quote,
  List,
  ListOrdered,
  Highlighter,
  AlignLeft,
  AlignCenter,
  Undo,
  Redo,
  Code,
} from 'lucide-react';
import {
  createSermonEditorExtensions,
  getEditorMarkdown,
  registerSermonEditor,
} from '@/editor/transport/markdownTransport';

interface TipTapEditorProps {
  /** Canonical Markdown (SermonDocument.body). */
  content: string;
  /** Receives canonical Markdown on every update. */
  onChange: (markdown: string) => void;
}

interface ToolbarButtonProps {
  onClick: () => void;
  active?: boolean;
  disabled?: boolean;
  title: string;
  children: React.ReactNode;
}

function ToolbarButton({ onClick, active, disabled, title, children }: ToolbarButtonProps) {
  return (
    <button
      onMouseDown={(e) => {
        e.preventDefault();
        onClick();
      }}
      disabled={disabled}
      title={title}
      className={[
        'flex items-center justify-center w-7 h-7 rounded transition-all duration-100',
        active
          ? 'bg-primary/20 text-primary' :'text-muted-foreground hover:bg-surface-3 hover:text-foreground',
        disabled ? 'opacity-30 cursor-not-allowed' : 'cursor-pointer',
      ].join(' ')}
    >
      {children}
    </button>
  );
}

export default function TipTapEditor({ content, onChange }: TipTapEditorProps) {
  const editor = useEditor({
    extensions: createSermonEditorExtensions(),
    content,
    editorProps: {
      attributes: {
        class: 'editor-prose min-h-[500px] focus:outline-none',
      },
    },
    onUpdate: ({ editor: ed }) => {
      // Canonical Markdown out — this is exactly what save will persist.
      onChange(getEditorMarkdown(ed));
    },
    immediatelyRender: false,
  });

  // Register the live instance so the lint seam reads the editor's real
  // state and lint findings can navigate to source positions.
  React.useEffect(() => {
    registerSermonEditor(editor);
    return () => registerSermonEditor(null);
  }, [editor]);

  if (!editor) return null;

  return (
    <div className="flex flex-col gap-0">
      {/* Toolbar */}
      <div className="flex items-center flex-wrap gap-0.5 pb-3 mb-1 border-b border-border/50">
        <ToolbarButton
          onClick={() => editor.chain().focus().toggleBold().run()}
          active={editor.isActive('bold')}
          title="Bold (Ctrl+B)"
        >
          <Bold size={13} />
        </ToolbarButton>
        <ToolbarButton
          onClick={() => editor.chain().focus().toggleItalic().run()}
          active={editor.isActive('italic')}
          title="Italic (Ctrl+I)"
        >
          <Italic size={13} />
        </ToolbarButton>
        <ToolbarButton
          onClick={() => editor.chain().focus().toggleUnderline().run()}
          active={editor.isActive('underline')}
          title="Underline (Ctrl+U)"
        >
          <UnderlineIcon size={13} />
        </ToolbarButton>
        <ToolbarButton
          onClick={() => editor.chain().focus().toggleHighlight().run()}
          active={editor.isActive('highlight')}
          title="Highlight"
        >
          <Highlighter size={13} />
        </ToolbarButton>
        <div className="w-px h-5 bg-border mx-1" />
        <ToolbarButton
          onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()}
          active={editor.isActive('heading', { level: 1 })}
          title="Heading 1"
        >
          <Heading1 size={13} />
        </ToolbarButton>
        <ToolbarButton
          onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
          active={editor.isActive('heading', { level: 2 })}
          title="Heading 2"
        >
          <Heading2 size={13} />
        </ToolbarButton>
        <ToolbarButton
          onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()}
          active={editor.isActive('heading', { level: 3 })}
          title="Heading 3"
        >
          <Heading3 size={13} />
        </ToolbarButton>
        <div className="w-px h-5 bg-border mx-1" />
        <ToolbarButton
          onClick={() => editor.chain().focus().toggleBlockquote().run()}
          active={editor.isActive('blockquote')}
          title="Blockquote — for scripture quotations"
        >
          <Quote size={13} />
        </ToolbarButton>
        <ToolbarButton
          onClick={() => editor.chain().focus().toggleBulletList().run()}
          active={editor.isActive('bulletList')}
          title="Bullet list"
        >
          <List size={13} />
        </ToolbarButton>
        <ToolbarButton
          onClick={() => editor.chain().focus().toggleOrderedList().run()}
          active={editor.isActive('orderedList')}
          title="Numbered list"
        >
          <ListOrdered size={13} />
        </ToolbarButton>
        <ToolbarButton
          onClick={() => editor.chain().focus().toggleCode().run()}
          active={editor.isActive('code')}
          title="Inline code"
        >
          <Code size={13} />
        </ToolbarButton>
        <div className="w-px h-5 bg-border mx-1" />
        <ToolbarButton
          onClick={() => editor.chain().focus().setTextAlign('left').run()}
          active={editor.isActive({ textAlign: 'left' })}
          title="Align left"
        >
          <AlignLeft size={13} />
        </ToolbarButton>
        <ToolbarButton
          onClick={() => editor.chain().focus().setTextAlign('center').run()}
          active={editor.isActive({ textAlign: 'center' })}
          title="Align center"
        >
          <AlignCenter size={13} />
        </ToolbarButton>
        <div className="w-px h-5 bg-border mx-1" />
        <ToolbarButton
          onClick={() => editor.chain().focus().undo().run()}
          disabled={!editor.can().undo()}
          title="Undo (Ctrl+Z)"
        >
          <Undo size={13} />
        </ToolbarButton>
        <ToolbarButton
          onClick={() => editor.chain().focus().redo().run()}
          disabled={!editor.can().redo()}
          title="Redo (Ctrl+Shift+Z)"
        >
          <Redo size={13} />
        </ToolbarButton>
      </div>

      {/* Editor content */}
      <EditorContent editor={editor} />
    </div>
  );
}