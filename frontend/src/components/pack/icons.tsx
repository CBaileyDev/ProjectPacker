import {
  Activity,
  AlertCircle,
  Check,
  ChevronDown,
  Clock,
  Copy,
  Eye,
  EyeOff,
  File,
  FileText,
  Folder,
  FolderOpen,
  Github,
  Keyboard,
  Loader2,
  Lock,
  type LucideProps,
  Package,
  Play,
  RefreshCw,
  Save,
  Search,
  Settings,
  Sliders,
  Sparkles,
  Star,
  X,
  Zap,
} from "lucide-react";
import type { ComponentType } from "react";

/**
 * Stroke 1.5 + 16 px size match the Linear / Vercel aesthetic the app
 * targets. Callers can still override either via props.
 */
const DEFAULTS: Partial<LucideProps> = {
  size: 16,
  strokeWidth: 1.5,
};

function wrap(Icon: ComponentType<LucideProps>): ComponentType<LucideProps> {
  return function WrappedIcon(props: LucideProps) {
    return <Icon {...DEFAULTS} {...props} />;
  };
}

export const FolderIcon = wrap(Folder);
export const GithubIcon = wrap(Github);
export const CopyIcon = wrap(Copy);
export const SaveIcon = wrap(Save);
export const CheckIcon = wrap(Check);
export const XIcon = wrap(X);
export const AlertIcon = wrap(AlertCircle);
export const SparklesIcon = wrap(Sparkles);
export const ChevronDownIcon = wrap(ChevronDown);
export const PlayIcon = wrap(Play);
export const LoaderIcon = wrap(Loader2);
export const PackageIcon = wrap(Package);
export const FileIcon = wrap(File);
export const ClockIcon = wrap(Clock);
export const ZapIcon = wrap(Zap);
export const FolderOpenIcon = wrap(FolderOpen);
export const KeyboardIcon = wrap(Keyboard);
export const SlidersIcon = wrap(Sliders);
export const ActivityIcon = wrap(Activity);
export const FileTextIcon = wrap(FileText);
export const SettingsIcon = wrap(Settings);
export const StarIcon = wrap(Star);
export const LockIcon = wrap(Lock);
export const EyeIcon = wrap(Eye);
export const EyeOffIcon = wrap(EyeOff);
export const SearchIcon = wrap(Search);
export const RefreshIcon = wrap(RefreshCw);
