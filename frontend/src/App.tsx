import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { marked } from "marked";
import * as XLSX from "xlsx-js-style";
import { ModernDatePicker, ModernTimePicker } from "./DateTimePicker";
import {
  Play,
  Pause,
  Square,
  Mic,
  Sparkles,
  Plus,
  Clock,
  FileText,
  Volume2,
  Trash2,
  CheckCircle2,
  Settings,
  BarChart3,
  Layers,
  Copy,
  Calendar,
  ChevronLeft,
  ChevronRight,
  Upload,
  PlusCircle,
  CalendarDays,
  List,
  StickyNote,
  CheckCheck,
  FileSpreadsheet,
  Filter,
  Download,
  Eye,
  EyeOff,
  GitBranch,
  FolderOpen,
  Edit3,
  FileDown,
  ChevronDown,
  ChevronUp,
  RefreshCw,
  RotateCw,
  AlertCircle,
  Loader2
} from "lucide-react";

interface Project {
  id: string;
  name: string;
  client: string;
  rate_info: string;
  timer_text: string;
  is_running: boolean;
  is_paused: boolean;
  today_duration: string;
  today_days: string;
  entries_count: number;
  currency: string;
  daily_rate: number;
  hourly_rate: number;
  billing_type: string;
  rounding_mode?: string;
  min_granularity?: number;
  git_repo_path?: string;
}

interface AiTimeSlotOption {
  label: string;
  start: string;
  end: string;
}

interface GitSmartFillDto {
  success: boolean;
  repo_path: string;
  summary_text: string;
  commits_count: number;
  commits: string[];
  suggested_start_time?: string;
  suggested_end_time?: string;
  diff_stat?: string;
  is_fallback_to_day: boolean;
  ai_reason?: string;
  time_slots?: AiTimeSlotOption[];
  message: string;
}

interface BatchSmartEntry {
  temp_id: string;
  date: string;
  start_time: string;
  end_time: string;
  summary: string;
  reason: string;
  commits_count: number;
  is_selected: boolean;
}

interface BatchSmartFillResult {
  success: boolean;
  total_days: number;
  total_commits: number;
  entries: BatchSmartEntry[];
  message: string;
}

interface AudioClip {
  id: string;
  clip_order: number;
  audio_path: string;
  duration_secs: number;
  duration_text: string;
  transcript?: string;
  status: string;
}

interface MeetingDetail {
  id: string;
  topic: string;
  start_time: string;
  end_time?: string;
  duration_text: string;
  status: string;
  summary?: string;
  quick_notes?: string;
  clips: AudioClip[];
}

interface Entry {
  id: string;
  project_id: string;
  project_name: string;
  duration_text: string;
  days_text: string;
  amount_text: string;
  summary_text: string;
  time_range: string;
  date_str: string;
  has_audio: boolean;
  audio_path: string;
  raw_secs: number;
  is_invoiced: boolean;
  status?: "normal" | "recording" | "summarizing" | string;
  meetings?: MeetingDetail[];
}

interface DayGroup {
  date_str: string;
  display_date: string;
  entries_count: number;
  total_duration: string;
  total_days: string;
  total_amount: string;
  entries: Entry[];
}

interface GlobalStats {
  total_days: string;
  total_amount: string;
  running_count: number;
  is_recording_audio: boolean;
  active_recording_project_id?: string;
  active_recording_project_name: string;
  is_audio_paused: boolean;
}

interface StopMeetingResult {
  success: boolean;
  project_id: string;
  topic: string;
  audio_path: string;
  transcript: string;
  summary: string;
  message: string;
}

interface Note {
  id: string;
  project_id: string;
  content: string;
  time_text: string;
}

export function App() {
  const [activeTab, setActiveTab] = useState<"workspace" | "stats" | "settings">("workspace");
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string>("");
  const [dayGroups, setDayGroups] = useState<DayGroup[]>([]);
  const [notes, setNotes] = useState<Note[]>([]);
  const [globalStats, setGlobalStats] = useState<GlobalStats>({
    total_days: "0.00 人天",
    total_amount: "¥ 0.00",
    running_count: 0,
    is_recording_audio: false,
    active_recording_project_name: "",
    is_audio_paused: false,
  });

  const [noteInput, setNoteInput] = useState("");
  interface ToastInfo {
    message: string;
    actionLabel?: string;
    onAction?: () => void;
  }
  const [toast, setToast] = useState<ToastInfo | null>(null);

  const showToast = (msg: string, actionLabel?: string, onAction?: () => void) => {
    setToast({ message: msg, actionLabel, onAction });
    setTimeout(() => {
      setToast((curr) => (curr?.message === msg ? null : curr));
    }, onAction ? 8000 : 3500);
  };

  // Month & View mode
  const now = new Date();
  const currentMonthStr = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;

  // Modals
  const [showMeetingModal, setShowMeetingModal] = useState(false);
  const [showNewProjModal, setShowNewProjModal] = useState(false);
  const [showDetailModal, setShowDetailModal] = useState(false);
  const [showManualEntryModal, setShowManualEntryModal] = useState(false);
  const [showManualMeetingModal, setShowManualMeetingModal] = useState(false);
  const [showNoteModal, setShowNoteModal] = useState(false);
  const [showFilterModal, setShowFilterModal] = useState(false);
  const [showReportExportModal, setShowReportExportModal] = useState(false);
  const [showExportSettingsModal, setShowExportSettingsModal] = useState(false);
  const [exportStrategy, setExportStrategy] = useState<"rebalance" | "min8_carry" | "cap8_carry" | "exact">("rebalance");
  const [exportSelectedProjects, setExportSelectedProjects] = useState<string[]>([]);
  const [exportSelectedMonths, setExportSelectedMonths] = useState<string[]>([currentMonthStr]);
  const [exportIncludeAmount, setExportIncludeAmount] = useState<boolean>(true); // 是否在 Excel 中导出金额
  const [hideAmount, setHideAmount] = useState<boolean>(() => {
    return localStorage.getItem("mythmanday_hide_amount") === "true";
  }); // 全局隐藏金额模式
  const [isExportingExcel, setIsExportingExcel] = useState(false);
  const [selectedEntry, setSelectedEntry] = useState<Entry | null>(null);

  const [selectedMonth, setSelectedMonth] = useState<string>(currentMonthStr);
  const [viewMode, setViewMode] = useState<"list" | "calendar">("list");
  const [calendarSelectedDate, setCalendarSelectedDate] = useState<string | null>(null);
  const [reportViewMode, setReportViewMode] = useState<"preview" | "raw">("preview");

  // Form states - New Project & 商业计费规则
  const [newProjName, setNewProjName] = useState("");
  const [newProjClient, setNewProjClient] = useState("");
  const [newProjBillingType, setNewProjBillingType] = useState<"daily" | "hourly">("daily");
  const [newProjRate, setNewProjRate] = useState("3000");
  const [newProjCurrency, setNewProjCurrency] = useState("CNY");
  const [newProjRoundingMode, setNewProjRoundingMode] = useState<"ceil" | "half_up" | "floor">("ceil");
  const [newProjMinGranularity, setNewProjMinGranularity] = useState("0.5");
  const [newProjGit, setNewProjGit] = useState("");

  // Form states - Edit Project (老项目规则配置)
  const [showEditProjModal, setShowEditProjModal] = useState(false);
  const [editProjId, setEditProjId] = useState("");
  const [editProjName, setEditProjName] = useState("");
  const [editProjClient, setEditProjClient] = useState("");
  const [editProjBillingType, setEditProjBillingType] = useState<"daily" | "hourly">("daily");
  const [editProjRate, setEditProjRate] = useState("3000");
  const [editProjCurrency, setEditProjCurrency] = useState("CNY");
  const [editProjRoundingMode, setEditProjRoundingMode] = useState<"ceil" | "half_up" | "floor">("ceil");
  const [editProjMinGranularity, setEditProjMinGranularity] = useState("0.5");
  const [editProjGit, setEditProjGit] = useState("");
  const [projectToDelete, setProjectToDelete] = useState<Project | null>(null);
  const [isDeletingProject, setIsDeletingProject] = useState(false);

  // Meeting Form (Quick Timer)
  const [meetingTopic, setMeetingTopic] = useState("");
  const [meetingRecordAudio, setMeetingRecordAudio] = useState(true);
  const [meetingSelectedPid, setMeetingSelectedPid] = useState<string>("");

  // Manual Log Entry Form
  const [manualEntryDate, setManualEntryDate] = useState(new Date().toISOString().split("T")[0]);
  const [manualEntryStartTime, setManualEntryStartTime] = useState("09:30");
  const [manualEntryEndTime, setManualEntryEndTime] = useState("12:30");
  const [manualEntrySummary, setManualEntrySummary] = useState("");
  const [manualEntryRepoPath, setManualEntryRepoPath] = useState("");
  const [isSmartFilling, setIsSmartFilling] = useState(false);
  const [showEditRepoInput, setShowEditRepoInput] = useState(false);
  const [smartAiReason, setSmartAiReason] = useState<string | null>(null);
  const [smartTimeSlots, setSmartTimeSlots] = useState<AiTimeSlotOption[]>([]);

  // Manual Log Meeting Form (with Audio upload)
  const [manualMeetingDate, setManualMeetingDate] = useState(new Date().toISOString().split("T")[0]);
  const [manualMeetingStartTime, setManualMeetingStartTime] = useState("14:00");
  const [manualMeetingEndTime, setManualMeetingEndTime] = useState("15:30");
  const [manualMeetingTopic, setManualMeetingTopic] = useState("");
  const [manualMeetingRate, setManualMeetingRate] = useState("350");
  const [manualMeetingPid, setManualMeetingPid] = useState<string>("");
  const [manualMeetingAudioPath, setManualMeetingAudioPath] = useState("");

  // Batch Smart Fill Modal States
  const [showBatchModal, setShowBatchModal] = useState(false);
  const [batchTab, setBatchTab] = useState<"git" | "text">("git");
  const [batchStartDate, setBatchStartDate] = useState(() => {
    const d = new Date();
    d.setDate(d.getDate() - 7);
    return d.toISOString().split("T")[0];
  });
  const [batchEndDate, setBatchEndDate] = useState(new Date().toISOString().split("T")[0]);
  const [batchRawText, setBatchRawText] = useState("");
  const [batchRepoPath, setBatchRepoPath] = useState("");
  const [isBatchScanning, setIsBatchScanning] = useState(false);
  const [isBatchSaving, setIsBatchSaving] = useState(false);
  const [batchEntries, setBatchEntries] = useState<BatchSmartEntry[]>([]);

  // Meeting Summary Modal (Stop Meeting & AI Generated Minutes)
  const [showMeetingSummaryModal, setShowMeetingSummaryModal] = useState(false);
  const [isStoppingMeeting, setIsStoppingMeeting] = useState(false);
  const [meetingSummaryData, setMeetingSummaryData] = useState<{
    project_id: string;
    topic: string;
    audio_path: string;
    summary: string;
    transcript: string;
  } | null>(null);

  // Global In-App Audio Player Modal State
  const [activePlayerAudio, setActivePlayerAudio] = useState<{
    path: string;
    title: string;
    dataUrl?: string;
    loading: boolean;
  } | null>(null);

  // Full Transcript Modal State
  const [activeTranscriptModal, setActiveTranscriptModal] = useState<{
    title: string;
    transcript: string;
  } | null>(null);

  // Edit Summary Modal State (for user modification before feeding to Agent)
  const [activeEditSummaryModal, setActiveEditSummaryModal] = useState<{
    entry_id: string;
    title: string;
    project_name?: string;
    date_str?: string;
    summary: string;
    raw_transcript: string;
  } | null>(null);
  const [isExportingPdf, setIsExportingPdf] = useState(false);
  const [isExtractingGit, setIsExtractingGit] = useState(false);
  // 跟踪哪些长纪要/长文本卡片已被用户点击「展开完整内容」
  const [expandedSummaryEntryIds, setExpandedSummaryEntryIds] = useState<Record<string, boolean>>({});
  // 跟踪正在重新转写的录音片段 ID
  const [retryingClipIds, setRetryingClipIds] = useState<Set<string>>(new Set());

  const toggleSummaryExpand = (entryId: string) => {
    setExpandedSummaryEntryIds((prev) => ({
      ...prev,
      [entryId]: !prev[entryId],
    }));
  };

  // Helper to separate concise summary from verbose raw transcript
  const splitSummaryAndTranscript = (fullText: string) => {
    if (!fullText) return { summary: "", transcript: "" };
    const markers = ["【会议录音全量转写实录】：", "【会议录音全量转写文稿】：", "【转写全文实录】", "【会议转写实录】"];
    for (const marker of markers) {
      const idx = fullText.indexOf(marker);
      if (idx !== -1) {
        return {
          summary: fullText.substring(0, idx).trim().replace(/---\s*$/, "").trim(),
          transcript: fullText.substring(idx + marker.length).trim(),
        };
      }
    }
    return { summary: fullText, transcript: "" };
  };

  // Settings Tab Form
  const [settingAiProvider, setSettingAiProvider] = useState("agy");
  const [settingAiBase, setSettingAiBase] = useState("https://api.openai.com/v1");
  const [settingAiKey, setSettingAiKey] = useState("");
  const [settingAiModel, setSettingAiModel] = useState("gpt-4o-mini");
  const [settingAiProxy, setSettingAiProxy] = useState("");
  const [settingHoursPerDay, setSettingHoursPerDay] = useState("8.0");
  const [settingAfkMinutes, setSettingAfkMinutes] = useState("10");
  const [isSavingSettings, setIsSavingSettings] = useState(false);

  const loadSettings = async () => {
    try {
      const res = await invoke<any>("get_settings");
      if (res) {
        if (res.ai_provider) setSettingAiProvider(res.ai_provider);
        if (res.ai_base) setSettingAiBase(res.ai_base);
        if (res.ai_key !== undefined) setSettingAiKey(res.ai_key);
        if (res.ai_model) setSettingAiModel(res.ai_model);
        if (res.ai_proxy !== undefined) setSettingAiProxy(res.ai_proxy);
        if (res.hours_per_day) setSettingHoursPerDay(res.hours_per_day);
        if (res.afk_minutes) setSettingAfkMinutes(res.afk_minutes);
      }
    } catch (e) {
      console.error("加载设置失败:", e);
    }
  };

  const handleSaveSettings = async () => {
    setIsSavingSettings(true);
    try {
      await invoke("save_settings", {
        aiProvider: settingAiProvider,
        aiBase: settingAiBase,
        aiKey: settingAiKey,
        aiModel: settingAiModel,
        aiProxy: settingAiProxy.trim() || null,
        hoursPerDay: settingHoursPerDay,
        afkMinutes: settingAfkMinutes,
      });
      showToast("✅ 系统设置与 AI 代理配置已保存成功！");
    } catch (e: any) {
      showToast("保存失败: " + e.toString());
    } finally {
      setIsSavingSettings(false);
    }
  };

  const renderMarkdown = (content: string) => {
    if (!content) return "";
    try {
      return marked.parse(content, { breaks: true, gfm: true }) as string;
    } catch {
      return content;
    }
  };


  const refreshData = async () => {
    try {
      const projs = await invoke<Project[]>("get_projects");
      setProjects(projs);
      if (!selectedProjectId && projs.length > 0) {
        setSelectedProjectId(projs[0].id);
      }
      const stats = await invoke<GlobalStats>("get_global_stats");
      setGlobalStats(stats);
    } catch (e) {
      console.error(e);
    }
  };

  const refreshProjectDetails = async (pid: string, mStr: string = selectedMonth) => {
    if (!pid) return;
    try {
      const groups = await invoke<DayGroup[]>("get_daily_groups", {
        projectId: pid,
        month: mStr || null,
      });
      setDayGroups(groups);
      const projNotes = await invoke<Note[]>("list_project_notes", { projectId: pid });
      setNotes(projNotes);
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    refreshData();
    loadSettings();
    const interval = setInterval(() => {
      refreshData();
      if (selectedProjectId) {
        refreshProjectDetails(selectedProjectId, selectedMonth);
      }
    }, 1000);
    return () => clearInterval(interval);
  }, [selectedProjectId, selectedMonth]);

  useEffect(() => {
    let unlistenFn: (() => void) | undefined;
    listen<any>("meeting_summary_ready", (event) => {
      const data = event.payload;
      showToast(`🎉 会议【${data.topic}】AI 纪要已提炼就绪！已更新落盘。`);
      refreshData();
      if (selectedProjectId) {
        refreshProjectDetails(selectedProjectId, selectedMonth);
      }
      // 自动弹出纪要确认/编辑弹窗，让用户第一时间查看和微调
      setMeetingSummaryData({
        project_id: data.project_id,
        topic: data.topic,
        audio_path: data.audio_path,
        summary: data.summary,
        transcript: data.transcript,
      });
      setShowMeetingSummaryModal(true);
    }).then((unlisten) => {
      unlistenFn = unlisten;
    }).catch(console.error);

    let unlistenClipFn: (() => void) | undefined;
    listen<any>("meeting_clip_ready", () => {
      setRetryingClipIds(new Set());
      refreshData();
      if (selectedProjectId) {
        refreshProjectDetails(selectedProjectId, selectedMonth);
      }
    }).then((unlisten) => {
      unlistenClipFn = unlisten;
    }).catch(console.error);

    return () => {
      if (unlistenFn) unlistenFn();
      if (unlistenClipFn) unlistenClipFn();
    };
  }, [selectedProjectId, selectedMonth]);

  const handlePrevMonth = () => {
    const [y, m] = selectedMonth.split("-").map(Number);
    const prevDate = new Date(y, m - 2, 1);
    const pStr = `${prevDate.getFullYear()}-${String(prevDate.getMonth() + 1).padStart(2, "0")}`;
    setSelectedMonth(pStr);
  };

  const handleNextMonth = () => {
    const [y, m] = selectedMonth.split("-").map(Number);
    const nextDate = new Date(y, m, 1);
    const nStr = `${nextDate.getFullYear()}-${String(nextDate.getMonth() + 1).padStart(2, "0")}`;
    setSelectedMonth(nStr);
  };

  const handleLogManualEntry = async () => {
    if (!selectedProjectId) {
      showToast("请先选择关联项目");
      return;
    }
    try {
      const res = await invoke<string>("log_manual_entry", {
        projectId: selectedProjectId,
        date: manualEntryDate,
        startTime: manualEntryStartTime,
        endTime: manualEntryEndTime,
        summary: manualEntrySummary,
      });
      setShowManualEntryModal(false);
      setManualEntrySummary("");
      showToast(res);
      refreshData();
      refreshProjectDetails(selectedProjectId, selectedMonth);
    } catch (e: any) {
      showToast("补录失败: " + e.toString());
    }
  };

  const handleLogManualMeeting = async () => {
    try {
      const res = await invoke<string>("log_manual_meeting", {
        projectId: manualMeetingPid || null,
        date: manualMeetingDate,
        startTime: manualMeetingStartTime,
        endTime: manualMeetingEndTime,
        topic: manualMeetingTopic || "技术方案沟通会",
        rate: parseFloat(manualMeetingRate) || 350,
        sourceAudioPath: manualMeetingAudioPath.trim() || null,
      });
      setShowManualMeetingModal(false);
      setManualMeetingTopic("");
      setManualMeetingAudioPath("");
      showToast(res);
      refreshData();
      if (selectedProjectId) refreshProjectDetails(selectedProjectId, selectedMonth);
    } catch (e: any) {
      showToast("补录失败: " + e.toString());
    }
  };

  const activeProject = projects.find((p) => p.id === selectedProjectId);

  useEffect(() => {
    if (showManualEntryModal && activeProject) {
      setManualEntryRepoPath(activeProject.git_repo_path || "");
      setShowEditRepoInput(!activeProject.git_repo_path);
    }
  }, [showManualEntryModal, activeProject?.id, activeProject?.git_repo_path]);

  const handleSmartFillGit = async () => {
    if (!activeProject) return;
    setIsSmartFilling(true);
    try {
      const res = await invoke<GitSmartFillDto>("fetch_git_work_summary", {
        projectId: activeProject.id,
        date: manualEntryDate,
        startTime: manualEntryStartTime || null,
        endTime: manualEntryEndTime || null,
        customRepoPath: manualEntryRepoPath.trim() || null,
      });

      if (!res.success) {
        showToast("⚠️ " + res.message);
        if (!activeProject.git_repo_path || !res.repo_path) {
          setShowEditRepoInput(true);
        }
        return;
      }

      setManualEntrySummary(res.summary_text);
      if (res.repo_path) {
        setManualEntryRepoPath(res.repo_path);
      }

      if (res.suggested_start_time && res.suggested_end_time) {
        setManualEntryStartTime(res.suggested_start_time);
        setManualEntryEndTime(res.suggested_end_time);
      }

      setSmartAiReason(res.ai_reason || null);
      setSmartTimeSlots(res.time_slots || []);

      showToast("✨ " + res.message);
      refreshData();
    } catch (e: any) {
      showToast("Git 提取失败: " + e.toString());
    } finally {
      setIsSmartFilling(false);
    }
  };

  useEffect(() => {
    if (showBatchModal && activeProject) {
      setBatchRepoPath(activeProject.git_repo_path || "");
    }
  }, [showBatchModal, activeProject?.id, activeProject?.git_repo_path]);

  // 1. 批量时间段检索 Git 提交
  const handleBatchScanGit = async () => {
    if (!activeProject) return;
    setIsBatchScanning(true);
    try {
      const res = await invoke<BatchSmartFillResult>("fetch_batch_git_work_summary", {
        projectId: activeProject.id,
        startDate: batchStartDate,
        endDate: batchEndDate,
        customRepoPath: batchRepoPath.trim() || null,
      });

      if (!res.success) {
        showToast("⚠️ " + res.message);
        return;
      }

      setBatchEntries(res.entries);
      showToast("✨ " + res.message);
    } catch (e: any) {
      showToast("Git 批量检索失败: " + e.toString());
    } finally {
      setIsBatchScanning(false);
    }
  };

  // 2. 批量解析第三方 AI 智能体总结/自由文本
  const handleBatchParseText = async () => {
    if (!activeProject) return;
    if (!batchRawText.trim()) {
      showToast("⚠️ 请先粘贴待解析的 AI 智能体工作总结或自由纪要文本");
      return;
    }
    setIsBatchScanning(true);
    try {
      const res = await invoke<BatchSmartFillResult>("parse_custom_batch_summary", {
        projectId: activeProject.id,
        rawText: batchRawText,
      });

      if (!res.success) {
        showToast("⚠️ " + res.message);
        return;
      }

      setBatchEntries(res.entries);
      showToast("✨ " + res.message);
    } catch (e: any) {
      showToast("AI 智能体总结解析失败: " + e.toString());
    } finally {
      setIsBatchScanning(false);
    }
  };

  // 3. 提交选中的条目执行批量入库
  const handleBatchSave = async () => {
    if (!activeProject) return;
    const selected = batchEntries.filter((e) => e.is_selected);
    if (selected.length === 0) {
      showToast("⚠️ 请至少勾选一条待补录的工时记录");
      return;
    }

    setIsBatchSaving(true);
    try {
      const payload = selected.map((item) => ({
        date: item.date,
        start_time: item.start_time,
        end_time: item.end_time,
        summary: item.summary,
      }));

      const res = await invoke<string>("log_batch_manual_entries", {
        projectId: activeProject.id,
        entries: payload,
      });

      showToast("🎉 " + res);
      setShowBatchModal(false);
      setBatchEntries([]);
      setBatchRawText("");
      refreshData();
      if (selectedProjectId) {
        refreshProjectDetails(selectedProjectId, selectedMonth);
      }
    } catch (e: any) {
      showToast("批量补录失败: " + e.toString());
    } finally {
      setIsBatchSaving(false);
    }
  };

  // 仅停止当前录音片段（录音附属于会议，单次停止只转写原始文本，绝不生成纪要、不关会议）
  const handleStopClip = async () => {
    try {
      showToast("⏹ 正在停止并封包当前录音片段...");
      const res = await invoke<string>("stop_recording_clip");
      showToast("🎙️ " + res);
      refreshData();
      if (selectedProjectId) refreshProjectDetails(selectedProjectId, selectedMonth);
    } catch (e: any) {
      showToast("停止录音片段失败: " + e.toString());
    }
  };

  // 为当前会议开启新的录音片段
  const handleStartClipForMeeting = async (meetingId: string) => {
    try {
      const res = await invoke<string>("start_recording_for_meeting", { meetingId });
      showToast("🎙️ " + res);
      refreshData();
      if (selectedProjectId) refreshProjectDetails(selectedProjectId, selectedMonth);
    } catch (e: any) {
      showToast("启动新录音片段失败: " + e.toString());
    }
  };

  // 重新触发指定录音片段的语音转写
  const handleRetryTranscribeClip = async (clipId: string) => {
    setRetryingClipIds((prev) => new Set(prev).add(clipId));
    showToast("🎙️ 正在启动录音片段重新转写识别...");
    try {
      const msg = await invoke<string>("retry_transcribe_audio_clip", { clipId });
      showToast("✨ " + msg);
      refreshData();
      if (selectedProjectId) refreshProjectDetails(selectedProjectId, selectedMonth);
    } catch (e: any) {
      showToast("重新转写启动失败: " + e.toString());
      setRetryingClipIds((prev) => {
        const next = new Set(prev);
        next.delete(clipId);
        return next;
      });
    }
  };

  // 结束会议（方案2双选项：参数 stopProjectTimer 决定是否同步停止主工时）
  const handleFinishMeeting = async (stopProjectTimer: boolean = false, meetingId?: string) => {
    setIsStoppingMeeting(true);
    showToast(stopProjectTimer ? "⏹ 正在结束会议并封包今日工时..." : "⏹ 正在结束会议（工时继续保持计时）...");
    try {
      const res = await invoke<StopMeetingResult>("finish_meeting", {
        meetingId: meetingId || null,
        stopProjectTimer,
      });
      if (res && res.success) {
        showToast("🎉 " + res.message);
        refreshData();
        if (selectedProjectId) {
          refreshProjectDetails(selectedProjectId, selectedMonth);
        }
      }
    } catch (e: any) {
      showToast("结束会议失败: " + e.toString());
    } finally {
      setIsStoppingMeeting(false);
    }
  };

  // 兼容老调用
  const handleStopMeeting = async () => {
    await handleFinishMeeting(false);
  };

  const handleSaveMeetingSummary = async () => {
    if (!meetingSummaryData) return;
    try {
      const res = await invoke<string>("save_meeting_summary_to_project", {
        projectId: meetingSummaryData.project_id,
        topic: meetingSummaryData.topic,
        summary: meetingSummaryData.summary,
        audioPath: meetingSummaryData.audio_path || null,
      });
      showToast("✅ " + res);
      setShowMeetingSummaryModal(false);
      setMeetingSummaryData(null);
      refreshData();
      if (selectedProjectId) {
        refreshProjectDetails(selectedProjectId, selectedMonth);
      }
    } catch (e: any) {
      showToast("保存会议纪要失败: " + e.toString());
    }
  };

  const handleStart = async (id: string) => {
    await invoke("start_timer", { projectId: id });
    showToast("▶ 工时通道已启动计时");
    refreshData();
  };

  const handlePause = async (id: string) => {
    await invoke("pause_timer", { projectId: id });
    showToast("⏸ 工时已挂起暂停");
    refreshData();
  };

  const handleResume = async (id: string) => {
    await invoke("resume_timer", { projectId: id });
    showToast("▶ 已继续工时计时");
    refreshData();
  };

  const handleStop = async (id: string) => {
    const res = await invoke<string>("stop_timer", { projectId: id });
    showToast(res);
    refreshData();
    if (selectedProjectId) refreshProjectDetails(selectedProjectId);
  };

  const handleAddNote = async () => {
    if (!noteInput.trim() || !selectedProjectId) return;
    await invoke("add_project_note", { projectId: selectedProjectId, content: noteInput });
    setNoteInput("");
    showToast("备忘速记已存入");
    refreshProjectDetails(selectedProjectId);
  };

  const handleDeleteNote = async (nid: string) => {
    await invoke("delete_note", { noteId: nid });
    showToast("备忘已删除");
    if (selectedProjectId) refreshProjectDetails(selectedProjectId);
  };

  const handleCreateProject = async () => {
    if (!newProjName.trim()) {
      showToast("项目名称不能为空");
      return;
    }
    try {
      const parsedRate = parseFloat(newProjRate) || 0;
      const parsedGran = parseFloat(newProjMinGranularity) || 0.5;

      const pid = await invoke<string>("create_project", {
        name: newProjName,
        client: newProjClient,
        billingType: newProjBillingType,
        rate: parsedRate,
        dailyRate: newProjBillingType === "daily" ? parsedRate : parsedRate * 8.0,
        currency: newProjCurrency.trim().toUpperCase() || "CNY",
        roundingMode: newProjRoundingMode,
        minGranularity: parsedGran,
        gitRepo: newProjGit || null,
      });
      setShowNewProjModal(false);
      setNewProjName("");
      setNewProjClient("");
      setNewProjRate(newProjBillingType === "daily" ? "3000" : "400");
      showToast("🎉 新商业项目与计费规则创建成功");
      setSelectedProjectId(pid);
      refreshData();
    } catch (e: any) {
      showToast(e.toString());
    }
  };

  const handleOpenEditProject = (proj: Project) => {
    setEditProjId(proj.id);
    setEditProjName(proj.name);
    setEditProjClient(proj.client || "");
    const isHourly = proj.billing_type === "hourly";
    setEditProjBillingType(isHourly ? "hourly" : "daily");
    setEditProjRate(isHourly ? (proj.hourly_rate ? proj.hourly_rate.toString() : "400") : (proj.daily_rate ? proj.daily_rate.toString() : "3000"));
    setEditProjCurrency(proj.currency || "CNY");
    setEditProjRoundingMode((proj.rounding_mode as any) || "ceil");
    setEditProjMinGranularity(proj.min_granularity !== undefined ? proj.min_granularity.toString() : "0.5");
    setEditProjGit(proj.git_repo_path || "");
    setShowEditProjModal(true);
  };

  const handleUpdateProject = async () => {
    if (!editProjId) return;
    if (!editProjName.trim()) {
      showToast("项目名称不能为空");
      return;
    }
    try {
      const parsedRate = parseFloat(editProjRate) || 0;
      const parsedGran = parseFloat(editProjMinGranularity) || 0.5;

      await invoke("update_project", {
        id: editProjId,
        name: editProjName,
        client: editProjClient,
        billingType: editProjBillingType,
        rate: parsedRate,
        currency: editProjCurrency.trim().toUpperCase() || "CNY",
        roundingMode: editProjRoundingMode,
        minGranularity: parsedGran,
        gitRepo: editProjGit ? editProjGit.trim() : null,
      });

      setShowEditProjModal(false);
      showToast("✅ 项目计费规则已更新并保存");
      await refreshData();
      if (selectedProjectId) {
        refreshProjectDetails(selectedProjectId);
      }
    } catch (e: any) {
      showToast("更新失败: " + e.toString());
    }
  };

  const handleDeleteProject = async (proj: Project) => {
    setIsDeletingProject(true);
    try {
      await invoke("delete_project", { id: proj.id });
      showToast(`🗑️ 项目「${proj.name}」已成功删除`);
      setProjectToDelete(null);
      setShowEditProjModal(false);

      const projs = await invoke<Project[]>("get_projects");
      setProjects(projs);
      if (selectedProjectId === proj.id) {
        if (projs.length > 0) {
          setSelectedProjectId(projs[0].id);
          refreshProjectDetails(projs[0].id, selectedMonth);
        } else {
          setSelectedProjectId("");
          setDayGroups([]);
        }
      } else {
        await refreshData();
      }
    } catch (e: any) {
      showToast("删除项目失败: " + e.toString());
    } finally {
      setIsDeletingProject(false);
    }
  };

  const handleStartMeeting = async () => {
    try {
      const res = await invoke<string>("start_quick_meeting", {
        projectId: meetingSelectedPid || null,
        topic: meetingTopic.trim() || "技术方案沟通评审",
        recordAudio: meetingRecordAudio,
      });
      setShowMeetingModal(false);
      setMeetingTopic("");
      showToast(res);
      refreshData();
    } catch (e: any) {
      showToast(e.toString());
    }
  };

  const handleTranscribe = async (entry: Entry) => {
    showToast("🤖 AI 正在离线/在线识别音频并总结纪要...");
    try {
      await invoke<string>("transcribe_entry_audio", {
        entryId: entry.id,
        audioPath: entry.audio_path,
      });
      showToast("AI 会议纪要提炼完成！");
      if (selectedProjectId) refreshProjectDetails(selectedProjectId);
    } catch (e: any) {
      showToast("转写失败: " + e.toString());
    }
  };

  const handleOpenAudioPlayer = async (audioPath: string, title: string = "会议录音凭证") => {
    setActivePlayerAudio({
      path: audioPath,
      title,
      loading: true,
    });
    try {
      const dataUrl = await invoke<string>("get_audio_data_url", { audioPath });
      setActivePlayerAudio({
        path: audioPath,
        title,
        dataUrl,
        loading: false,
      });
    } catch (e: any) {
      showToast("载入录音文件失败: " + e.toString());
      setActivePlayerAudio((prev) => (prev ? { ...prev, loading: false } : null));
    }
  };

  const handleSaveEntrySummary = async () => {
    if (!activeEditSummaryModal) return;
    try {
      const fullTextToSave = activeEditSummaryModal.raw_transcript
        ? `${activeEditSummaryModal.summary.trim()}\n\n---\n【会议录音全量转写实录】：\n${activeEditSummaryModal.raw_transcript.trim()}`
        : activeEditSummaryModal.summary.trim();

      const res = await invoke<string>("update_entry_summary", {
        entryId: activeEditSummaryModal.entry_id,
        summary: fullTextToSave,
      });

      showToast("✅ " + res);
      setActiveEditSummaryModal(null);
      if (selectedProjectId) {
        refreshProjectDetails(selectedProjectId, selectedMonth);
      }
    } catch (e: any) {
      showToast("保存失败: " + e.toString());
    }
  };

  const handleExportPdf = async (options: {
    title: string;
    projectName: string;
    dateStr: string;
    summaryContent: string;
  }) => {
    if (!options.summaryContent.trim()) {
      showToast("⚠️ 会议纪要内容为空，无法导出 PDF！");
      return;
    }
    setIsExportingPdf(true);
    showToast("📄 正在排版并生成客户级交付 PDF 纪要...");
    try {
      const sanitizedProj = options.projectName.replace(/[\\/:*?"<>|]/g, "_");
      const sanitizedTitle = options.title.replace(/[\\/:*?"<>|]/g, "_").replace(/^📅\s*/, "");
      const filename = `${options.dateStr}_${sanitizedProj}_${sanitizedTitle}_会议纪要.pdf`;
      const htmlBody = renderMarkdown(options.summaryContent);

      const savedPath = await invoke<string>("export_meeting_pdf", {
        filename,
        title: options.title.replace(/^📅\s*/, ""),
        projectName: options.projectName,
        dateStr: options.dateStr,
        htmlContent: htmlBody,
      });

      showToast(`🎉 PDF 纪要已导出至: ${savedPath}`);
    } catch (e: any) {
      showToast("PDF 导出失败: " + e.toString());
    } finally {
      setIsExportingPdf(false);
    }
  };

  interface ExtractGitResultDto {
    success: boolean;
    commits_count: number;
    commits: string[];
    diff_stat: string;
    extracted_text: string;
    message: string;
  }

  const handleExtractGitSummary = async (mode: "replace" | "append" = "replace") => {
    if (!activeEditSummaryModal) return;
    setIsExtractingGit(true);
    showToast("🔍 正在从项目 Git 仓库全量扫描并抽取提交记录...");
    try {
      const res = await invoke<ExtractGitResultDto>("extract_entry_git_summary", {
        entryId: activeEditSummaryModal.entry_id,
        customRepoPath: activeProject?.git_repo_path || null,
      });

      if (!res.success) {
        showToast("⚠️ " + res.message);
        return;
      }

      if (res.commits_count === 0 && !res.diff_stat) {
        showToast("ℹ️ " + res.message);
        return;
      }

      const newSummaryText = mode === "append" && activeEditSummaryModal.summary.trim()
        ? `${activeEditSummaryModal.summary.trim()}\n\n---\n${res.extracted_text}`
        : res.extracted_text;

      setActiveEditSummaryModal({
        ...activeEditSummaryModal,
        summary: newSummaryText,
      });

      showToast(`✨ ${res.message} 已自动填入纪要框`);
    } catch (e: any) {
      showToast("抽取 Git 提交失败: " + e.toString());
    } finally {
      setIsExtractingGit(false);
    }
  };

  // 结算状态与报表多维导出状态
  interface SettlementReportData {
    markdown: string;
    total_days: number;
    total_amount: number;
    settled_amount: number;
    pending_amount: number;
    matching_entry_ids: string[];
    entries: Entry[];
  }

  const [reportSelectedProjects, setReportSelectedProjects] = useState<string[]>([]);
  const [reportSelectedMonths, setReportSelectedMonths] = useState<string[]>([currentMonthStr]);
  const [reportStatusFilter, setReportStatusFilter] = useState<"all" | "pending" | "settled">("all");
  const [currentStrategy, setCurrentStrategy] = useState<"rebalance" | "min8_carry" | "cap8_carry" | "exact">("rebalance");
  const [settlementReport, setSettlementReport] = useState<SettlementReportData | null>(null);

  const handleToggleEntryInvoiced = async (entryId: string, currentStatus: boolean) => {
    try {
      await invoke("toggle_entry_invoiced", {
        entryId,
        isInvoiced: !currentStatus,
      });
      showToast(!currentStatus ? "✅ 已标记为已结算！" : "⏳ 已重置为待结算状态");
      if (selectedProjectId) refreshProjectDetails(selectedProjectId);
      if (activeTab === "stats") {
        fetchSettlementReport();
      }
    } catch (e: any) {
      showToast("操作失败: " + e.toString());
    }
  };

  const handleBatchToggleInvoiced = async (isInvoiced: boolean) => {
    if (!settlementReport || settlementReport.matching_entry_ids.length === 0) {
      showToast("当前无匹配条目可批量操作");
      return;
    }
    try {
      const count = await invoke<number>("batch_toggle_entries_invoiced", {
        entryIds: settlementReport.matching_entry_ids,
        isInvoiced,
      });
      showToast(`已将匹配的 ${count} 笔工时流水标记为「${isInvoiced ? "已结算" : "待结算"}」！`);
      if (selectedProjectId) refreshProjectDetails(selectedProjectId);
      fetchSettlementReport();
    } catch (e: any) {
      showToast("批量操作失败: " + e.toString());
    }
  };

  const fetchSettlementReport = async (overrideStrategy?: "rebalance" | "min8_carry" | "cap8_carry" | "exact") => {
    const strat = overrideStrategy || currentStrategy;
    try {
      const res = await invoke<SettlementReportData>("generate_settlement_report", {
        projectIds: reportSelectedProjects.length > 0 ? reportSelectedProjects : null,
        months: reportSelectedMonths.length > 0 ? reportSelectedMonths : null,
        statusFilter: reportStatusFilter,
        enableRebalance: strat === "rebalance",
        settlementStrategy: strat,
      });
      setSettlementReport(res);
    } catch (e: any) {
      showToast("生成结算报表失败: " + e.toString());
    }
  };

  // 格式化工作交付条目为规范换行编号
  const formatDeliverablesToNumberedLines = (rawSummary: string): string => {
    if (!rawSummary) return "持续代码开发与测试交付";
    const clean = rawSummary
      .replace(/\[削峰回填\]\s*/g, "")
      .replace(/\[超额顺延\]\s*/g, "");
    const parts = clean
      .split(/[；;。\n]+/)
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    if (parts.length === 0) return "持续代码开发与测试交付";
    if (parts.length === 1) return parts[0];
    return parts.map((p, idx) => `${idx + 1}. ${p}`).join("\n");
  };

  // 格式化投入工时：以小时为单位，最小刻度为0.5，不足0.5按0.5，超过按1 (纯数字，单位在表头注明)
  const formatToHalfHours = (rawText: string): string => {
    if (!rawText) return "0.0";
    // 提取数字并转换
    const cleanStr = rawText.replace(/小时/g, "").trim();
    if (cleanStr.includes(":")) {
      const parts = cleanStr.split(":").map((v) => parseInt(v, 10) || 0);
      let totalHours = 0;
      if (parts.length >= 2) {
        totalHours = parts[0] + parts[1] / 60 + (parts[2] || 0) / 3600;
      } else {
        totalHours = parseFloat(cleanStr) || 0;
      }
      if (totalHours <= 0) return "0.0";
      const stepped = Math.ceil(totalHours * 2) / 2;
      return stepped.toFixed(1);
    }

    const val = parseFloat(cleanStr);
    if (!isNaN(val) && val > 0) {
      const stepped = Math.ceil(val * 2) / 2;
      return stepped.toFixed(1);
    }
    return "0.0";
  };

  // 提取纯数字金额与天数字符串
  const formatPureNumber = (rawText: string): string => {
    if (!rawText) return "0.00";
    const match = rawText.match(/([\d.]+)/);
    if (match) {
      const v = parseFloat(match[1]);
      return isNaN(v) ? "0.00" : v.toFixed(2);
    }
    return "0.00";
  };

  // 核心多项目·多月份按Sheet专业 Excel 导出执行
  const executeExportExcel = async () => {
    const targetProjectIds = exportSelectedProjects.length > 0
      ? exportSelectedProjects
      : projects.map((p) => p.id);

    if (targetProjectIds.length === 0) {
      showToast("请先选择至少一个要导出的项目！");
      return;
    }

    const targetMonths = exportSelectedMonths.length > 0
      ? exportSelectedMonths
      : [currentMonthStr];

    setIsExportingExcel(true);
    setShowExportSettingsModal(false);
    showToast("📊 正在按项目生成独立对账单文件...");

    try {
      const dateTag = new Date().toISOString().split("T")[0];
      const filesPayload: { filename: string; base64_data: string }[] = [];

      // 遍历每个被选中的项目，单独生成一个专属的 Workbook
      for (const pid of targetProjectIds) {
        const proj = projects.find((p) => p.id === pid);
        if (!proj) continue;

        const workbook = XLSX.utils.book_new();

        // 针对该项目的各个选择月份，每个月生成一个单独的 Sheet
        for (const monthStr of targetMonths) {
          // 调用后端带策略引擎的数据接口
          const reportRes = await invoke<SettlementReportData>("generate_settlement_report", {
            projectIds: [pid],
            months: [monthStr],
            statusFilter: reportStatusFilter,
            enableRebalance: exportStrategy === "rebalance",
            settlementStrategy: exportStrategy,
          });

          // 如果该月没有记录，仍然生成一个清晰的标准空模板 Sheet
          const entries = reportRes.entries || [];

          // 1. 构建商务抬头部信息
          const strategyLabels: Record<string, string> = {
            rebalance: "合规削峰填谷 (单日8h封顶·工作日平摊)",
            min8_carry: "保底8小时模式 (不足8h按8h/天·超额顺延新天)",
            cap8_carry: "封顶顺延模式 (单日8h封顶·超额顺延新天)",
            exact: "实际物理工时模式 (实报实销·同日合并)",
          };

          // 1. 构建商务抬头部信息 (规范 2 列键值对排版，避免挤压串列)
          const colCount = exportIncludeAmount ? 8 : 7;
          const headerColumns = exportIncludeAmount
            ? ["序号", "核算日期", "时间范围", "投入工时 (小时)", "折算人天 (天)", `核算金额 (${proj.currency})`, "结算状态", "工作交付清单与产出成果"]
            : ["序号", "核算日期", "时间范围", "投入工时 (小时)", "折算人天 (天)", "结算状态", "工作交付清单与产出成果"];

          const sheetHeaderData = [
            ["【项目人天结算对账单】", ...Array(colCount - 1).fill("")],
            ["项目名称:", proj.name, "", "客户 / 委托方:", proj.client || "商业客户", "", "核算月份:", monthStr],
            ["结算标准:", exportIncludeAmount ? `${proj.daily_rate} ${proj.currency}/天 (8.0h/天)` : "8.0h/天 标准人天", "", "核算策略:", strategyLabels[exportStrategy], "", "结算状态:", reportStatusFilter === "all" ? "全部工时" : reportStatusFilter === "settled" ? "已结算" : "待结算"],
            ["导出日期:", dateTag, ...Array(colCount - 2).fill("")],
            [], // 空行分隔
            headerColumns,
          ];

          // 2. 排序并填充明细流水行 (正序升序：从月初到月末按时间先后排序，纯数字无冗余单位)
          const sortedEntries = [...entries].sort((a, b) => a.date_str.localeCompare(b.date_str));
          const sheetRows: any[][] = [];
          sortedEntries.forEach((item, idx) => {
            if (exportIncludeAmount) {
              sheetRows.push([
                idx + 1,
                item.date_str,
                item.time_range,
                parseFloat(formatToHalfHours(item.duration_text)),
                parseFloat(formatPureNumber(item.days_text)),
                parseFloat(formatPureNumber(item.amount_text)),
                item.is_invoiced ? "已结算" : "待结算",
                formatDeliverablesToNumberedLines(item.summary_text),
              ]);
            } else {
              sheetRows.push([
                idx + 1,
                item.date_str,
                item.time_range,
                parseFloat(formatToHalfHours(item.duration_text)),
                parseFloat(formatPureNumber(item.days_text)),
                item.is_invoiced ? "已结算" : "待结算",
                formatDeliverablesToNumberedLines(item.summary_text),
              ]);
            }
          });

          // 3. 构建汇总行
          const totalDays = sortedEntries.reduce((acc, curr) => {
            const v = parseFloat(curr.days_text.replace(/[^\d.]/g, "")) || 0;
            return acc + v;
          }, 0);

          const totalAmount = sortedEntries.reduce((acc, curr) => {
            const v = parseFloat(curr.amount_text.replace(/[^\d.]/g, "")) || 0;
            return acc + v;
          }, 0);

          sheetRows.push([]);
          if (exportIncludeAmount) {
            sheetRows.push([
              "【月度汇总】",
              `共计 ${sortedEntries.length} 个工作日`,
              "",
              "",
              parseFloat(totalDays.toFixed(2)),
              parseFloat(totalAmount.toFixed(2)),
              "",
              `核算规则: ${strategyLabels[exportStrategy]}`,
            ]);
          } else {
            sheetRows.push([
              "【月度汇总】",
              `共计 ${sortedEntries.length} 个工作日`,
              "",
              "",
              parseFloat(totalDays.toFixed(2)),
              "",
              `核算规则: ${strategyLabels[exportStrategy]}`,
            ]);
          }

          // 4. 将二维数据矩阵转为 Sheet
          const worksheet = XLSX.utils.aoa_to_sheet([...sheetHeaderData, ...sheetRows]);

          // 设置单元格合并 (Merges): 大标题跨列合并 A1:H1 或 A1:G1
          worksheet["!merges"] = [
            { s: { r: 0, c: 0 }, e: { r: 0, c: colCount - 1 } }, // 大标题合并
            { s: { r: 1, c: 1 }, e: { r: 1, c: 2 } }, // 项目名称跨2列
            { s: { r: 1, c: 4 }, e: { r: 1, c: 5 } }, // 客户跨2列
            { s: { r: 2, c: 1 }, e: { r: 2, c: 2 } }, // 结算标准跨2列
            { s: { r: 2, c: 4 }, e: { r: 2, c: 5 } }, // 核算策略跨2列
          ];

          // 5. 应用单元格样式与自动换行 (wrapText: true)
          // 遍历表头和数据行设置专业字体与对齐方式
          // 标题行样式 (Row 0: A1)
          const titleCell = worksheet["A1"];
          if (titleCell) {
            titleCell.s = {
              font: { name: "微软雅黑", sz: 16, bold: true, color: { rgb: "1E293B" } },
              alignment: { horizontal: "center", vertical: "center" },
            };
          }

          // 表头列名称 (Row 5: A6:H6 或 A6:G6)
          const colLetters = exportIncludeAmount
            ? ["A", "B", "C", "D", "E", "F", "G", "H"]
            : ["A", "B", "C", "D", "E", "F", "G"];
          const lastColLetter = colLetters[colLetters.length - 1];

          for (const col of colLetters) {
            const ref = `${col}6`;
            if (worksheet[ref]) {
              worksheet[ref].s = {
                font: { name: "微软雅黑", sz: 10, bold: true, color: { rgb: "334155" } },
                fill: { fgColor: { rgb: "F1F5F9" } },
                alignment: {
                  horizontal: col === lastColLetter ? "left" : "center",
                  vertical: "center",
                },
                border: {
                  top: { style: "thin", color: { rgb: "CBD5E1" } },
                  bottom: { style: "medium", color: { rgb: "94A3B8" } },
                },
              };
            }
          }

          // 数据行格式化 (从第7行开始，即 r = 6)
          const dataStartRow = 6;
          for (let i = 0; i < sortedEntries.length; i++) {
            const r = dataStartRow + i;

            for (let c = 0; c < colCount; c++) {
              const cellRef = XLSX.utils.encode_cell({ r, c });
              const cell = worksheet[cellRef];
              if (!cell) continue;

              if (c === colCount - 1) {
                // 最后一列: 工作交付清单与产出成果，设置自动换行和靠左居上
                cell.s = {
                  font: { name: "微软雅黑", sz: 10, color: { rgb: "1E293B" } },
                  alignment: {
                    wrapText: true,
                    vertical: "top",
                    horizontal: "left",
                  },
                  border: {
                    bottom: { style: "thin", color: { rgb: "E2E8F0" } },
                  },
                };
              } else {
                // 数字列 (D:投入工时, E:折算人天, [F:核算金额])
                let numFmt = undefined;
                if (c === 3) numFmt = "0.0";
                else if (c === 4) numFmt = "0.00";
                else if (exportIncludeAmount && c === 5) numFmt = "#,##0.00";

                const isStatusCol = exportIncludeAmount ? c === 6 : c === 5;

                cell.s = {
                  font: { name: "微软雅黑", sz: 10, color: { rgb: "334155" } },
                  alignment: {
                    vertical: "top",
                    horizontal: c === 0 || c === 1 || c === 2 || isStatusCol ? "center" : "right",
                  },
                  border: {
                    bottom: { style: "thin", color: { rgb: "E2E8F0" } },
                  },
                  ...(numFmt ? { numFmt } : {}),
                };
              }
            }
          }

          // 汇总行样式 (最后两行，即倒数第一行)
          const summaryRowIdx = dataStartRow + sortedEntries.length + 1;
          for (let c = 0; c < colCount; c++) {
            const cellRef = XLSX.utils.encode_cell({ r: summaryRowIdx, c });
            const cell = worksheet[cellRef];
            if (cell) {
              let numFmt = undefined;
              if (c === 4) numFmt = "0.00";
              else if (exportIncludeAmount && c === 5) numFmt = "#,##0.00";

              cell.s = {
                font: { name: "微软雅黑", sz: 10, bold: true, color: { rgb: "0F172A" } },
                fill: { fgColor: { rgb: "F8FAFC" } },
                alignment: {
                  vertical: "center",
                  horizontal: c === 4 || (exportIncludeAmount && c === 5) ? "right" : "left",
                },
                border: {
                  top: { style: "thin", color: { rgb: "CBD5E1" } },
                  bottom: { style: "double", color: { rgb: "64748B" } },
                },
                ...(numFmt ? { numFmt } : {}),
              };
            }
          }

          // 设置专业列宽 (自适应宽度，避免截断和挤压)
          worksheet["!cols"] = exportIncludeAmount
            ? [
                { wch: 10 },  // A: 序号
                { wch: 15 },  // B: 核算日期
                { wch: 15 },  // C: 时间范围
                { wch: 14 },  // D: 投入工时
                { wch: 14 },  // E: 折算人天
                { wch: 16 },  // F: 核算金额
                { wch: 12 },  // G: 结算状态
                { wch: 70 },  // H: 工作交付清单与产出成果
              ]
            : [
                { wch: 10 },  // A: 序号
                { wch: 15 },  // B: 核算日期
                { wch: 15 },  // C: 时间范围
                { wch: 14 },  // D: 投入工时
                { wch: 14 },  // E: 折算人天
                { wch: 12 },  // F: 结算状态
                { wch: 70 },  // G: 工作交付清单与产出成果
              ];

          // Sheet 命名为 "YYYY年MM月"
          const [y, m] = monthStr.split("-");
          const sheetName = `${y}年${m}月`;
          XLSX.utils.book_append_sheet(workbook, worksheet, sheetName);
        }

        // 文件命名规则：《项目名_人天结算对账单_导出日期.xlsx》
        const safeProjName = proj.name.replace(/[\\/:*?"<>|]/g, "_");
        const fileName = `${safeProjName}_人天结算对账单_${dateTag}.xlsx`;
        const b64Data = XLSX.write(workbook, { bookType: "xlsx", type: "base64" });

        filesPayload.push({
          filename: fileName,
          base64_data: b64Data,
        });
      }

      if (filesPayload.length === 0) {
        showToast("没有符合条件的对账单可供导出！");
        return;
      }

      // 保存全部文件并打开 Downloads 目录
      const savedPaths = await invoke<string[]>("save_multiple_excel_files", {
        files: filesPayload,
      });

      showToast(
        `🎉 成功导出 ${savedPaths.length} 个项目独立对账单 (每个月独立Sheet)！`,
        "📂 查看 Downloads 文件夹",
        () => {
          if (savedPaths.length > 0) {
            invoke("open_file_or_dir", { path: savedPaths[0] });
          }
        }
      );
    } catch (err: any) {
      showToast("导出失败: " + err.toString());
    } finally {
      setIsExportingExcel(false);
    }
  };

  return (
    <div className="flex h-screen w-screen bg-[#f1f5f9] text-slate-800 font-sans select-none antialiased overflow-hidden">
      {/* ---------------- 现代左侧导航侧边栏 (极简高级深灰调) ---------------- */}
      <aside className="w-60 bg-[#0f172a] text-slate-300 flex flex-col justify-between p-4 shrink-0 shadow-xl border-r border-slate-800/80">
        <div className="space-y-6">
          {/* 品牌标识 */}
          <div className="flex items-center gap-3 px-1 pt-1">
            <img 
              src="/logo.png" 
              alt="Logo" 
              className="w-8 h-8 rounded-lg shadow-sm shadow-indigo-500/30 object-contain bg-slate-900" 
            />
            <div>
              <h1 className="font-extrabold text-sm text-white tracking-wide">人天神话</h1>
              <p className="text-[10px] font-medium text-indigo-300/80 tracking-tight">The Mythical Man-Day</p>
            </div>
          </div>

          {/* 快捷主操作：发起会议沟通 */}
          <button
            onClick={() => setShowMeetingModal(true)}
            className="w-full h-9 px-3 bg-indigo-600 hover:bg-indigo-500 active:scale-[0.98] transition-all text-white font-medium text-xs rounded-lg flex items-center justify-center gap-2 shadow-sm shadow-indigo-600/30"
          >
            <Mic className="w-3.5 h-3.5" />
            发起会议沟通
          </button>

          {/* 导航菜单 */}
          <nav className="space-y-1">
            <div className="px-2.5 pb-1 text-[10px] font-semibold text-slate-500 uppercase tracking-wider">工作台</div>
            <button
              onClick={() => setActiveTab("workspace")}
              className={`w-full h-9 px-2.5 rounded-lg flex items-center justify-between text-xs font-medium transition-colors ${
                activeTab === "workspace"
                  ? "bg-slate-800 text-white shadow-xs"
                  : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200"
              }`}
            >
              <div className="flex items-center gap-2.5">
                <Layers className={`w-4 h-4 ${activeTab === "workspace" ? "text-indigo-400" : "text-slate-400"}`} />
                项目工作区
              </div>
              {globalStats.running_count > 0 && (
                <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
              )}
            </button>

            <button
              onClick={() => {
                setActiveTab("stats");
                fetchSettlementReport();
              }}
              className={`w-full h-9 px-2.5 rounded-lg flex items-center justify-between text-xs font-medium transition-colors ${
                activeTab === "stats"
                  ? "bg-slate-800 text-white shadow-xs"
                  : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200"
              }`}
            >
              <div className="flex items-center gap-2.5">
                <BarChart3 className={`w-4 h-4 ${activeTab === "stats" ? "text-indigo-400" : "text-slate-400"}`} />
                全局工时统计
              </div>
            </button>

            <button
              onClick={() => setActiveTab("settings")}
              className={`w-full h-9 px-2.5 rounded-lg flex items-center gap-2.5 text-xs font-medium transition-colors ${
                activeTab === "settings"
                  ? "bg-slate-800 text-white shadow-xs"
                  : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200"
              }`}
            >
              <Settings className={`w-4 h-4 ${activeTab === "settings" ? "text-indigo-400" : "text-slate-400"}`} />
              系统与AI引擎
            </button>
          </nav>
        </div>

        {/* 侧边栏底部累计投入指标 (选项 2: 仅专注人天与工时投入，隐去核算金额) */}
        <div className="p-3 bg-slate-800/80 rounded-xl border border-slate-700/60 shadow-inner space-y-1">
          <div className="flex items-center justify-between text-[11px] font-medium text-slate-400">
            <span>今日累计投入</span>
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-slate-700/70 text-indigo-300 font-mono">
              工时核算
            </span>
          </div>
          <div className="text-xl font-bold text-white tracking-tight">{globalStats.total_days}</div>
          <div className="text-[11px] text-slate-400 flex items-center justify-between pt-1 border-t border-slate-700/60">
            <span>运行通道</span>
            <span className="font-medium text-slate-300">
              {globalStats.running_count > 0 ? (
                <span className="text-emerald-400 font-semibold">{globalStats.running_count} 个进行中</span>
              ) : (
                "暂无进行中"
              )}
            </span>
          </div>
        </div>
      </aside>

      {/* ---------------- 主内容展示画布 ---------------- */}
      <main className="flex-1 flex flex-col min-w-0 bg-[#f1f5f9] overflow-hidden">
        {/* 顶部全局顶栏 */}
        <header className="h-14 border-b border-slate-200/90 bg-white px-6 flex items-center justify-between shrink-0 shadow-2xs">
          <div>
            <h2 className="text-sm font-bold text-slate-900">
              {activeTab === "workspace" && "项目工时工作区"}
              {activeTab === "stats" && "全局工时透视与技术日报"}
              {activeTab === "settings" && "系统设置与 AI 引擎配置"}
            </h2>
            <p className="text-[11px] text-slate-500">
              开发者人天中台 · 多通道并发工时 · 录音与交付凭证一体化沉淀
            </p>
          </div>

          <div className="flex items-center gap-3">
            {/* 活动通道徽章 */}
            <div
              className={`h-7 px-3 rounded-full border flex items-center gap-1.5 text-xs font-medium ${
                globalStats.running_count > 0
                  ? "bg-emerald-50 border-emerald-200 text-emerald-700"
                  : "bg-slate-100 border-slate-200 text-slate-600"
              }`}
            >
              <span
                className={`w-1.5 h-1.5 rounded-full ${
                  globalStats.running_count > 0 ? "bg-emerald-500 animate-pulse" : "bg-slate-400"
                }`}
              />
              {globalStats.running_count > 0 ? `${globalStats.running_count} 个通道计时中` : "无活动通道"}
            </div>

            {/* 全局金额隐私模式切换 */}
            <button
              onClick={() => {
                const nextVal = !hideAmount;
                setHideAmount(nextVal);
                localStorage.setItem("mythmanday_hide_amount", String(nextVal));
                showToast(nextVal ? "🔒 已开启隐私模式：全局隐藏金额" : "👁️ 已关闭隐私模式：正常显示金额");
              }}
              className={`h-8 px-2.5 rounded-lg border text-xs font-semibold flex items-center gap-1.5 transition-all shadow-xs cursor-pointer ${
                hideAmount
                  ? "bg-amber-50 border-amber-300 text-amber-800 hover:bg-amber-100"
                  : "bg-white border-slate-200 text-slate-700 hover:bg-slate-50"
              }`}
              title={hideAmount ? "当前已隐藏金额，点击恢复显示" : "点击开启隐私模式，全局隐藏金额"}
            >
              {hideAmount ? <EyeOff className="w-3.5 h-3.5 text-amber-600" /> : <Eye className="w-3.5 h-3.5 text-slate-500" />}
              <span>{hideAmount ? "金额已隐藏" : "显示金额"}</span>
            </button>

            <button
              onClick={() => setShowNewProjModal(true)}
              className="h-8 px-3 rounded-lg border border-slate-200 bg-white hover:bg-slate-50 text-slate-700 text-xs font-semibold flex items-center gap-1.5 shadow-xs transition-all cursor-pointer"
            >
              <Plus className="w-3.5 h-3.5 text-slate-500" />
              新建项目
            </button>
          </div>
        </header>

        {/* 录音微幅浮动横幅 */}
        {globalStats.is_recording_audio && (
          <div className="bg-amber-50 border-b border-amber-200 px-6 py-2 flex items-center justify-between text-xs font-medium text-amber-900">
            <div className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-amber-600 animate-ping" />
              <span>
                🎙️ 全程会议录音中: <strong className="font-bold">{globalStats.active_recording_project_name}</strong>
              </span>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={() => invoke("toggle_audio_pause")}
                className="px-2.5 py-1 bg-amber-600 hover:bg-amber-700 text-white rounded text-[11px] font-semibold transition-colors cursor-pointer"
              >
                {globalStats.is_audio_paused ? "▶ 继续录音" : "⏸ 暂停录音"}
              </button>
              <button
                onClick={handleStopClip}
                className="px-2.5 py-1 bg-amber-700 hover:bg-amber-800 text-white rounded text-[11px] font-semibold transition-colors cursor-pointer flex items-center gap-1 shadow-2xs"
                title="单独停止当前录音片段并转写文本（会议保持进行，后续可追加录音）"
              >
                <Square className="w-3 h-3 fill-current" />
                停止当前录音片段
              </button>
              <button
                onClick={() => handleFinishMeeting(false)}
                disabled={isStoppingMeeting}
                className="px-2.5 py-1 bg-indigo-600 hover:bg-indigo-700 text-white rounded text-[11px] font-semibold transition-colors cursor-pointer flex items-center gap-1 disabled:opacity-50 shadow-2xs"
                title="结束整场会议并提炼综合纪要（主工时通道继续保持计时）"
              >
                <Square className="w-3 h-3 fill-current" />
                {isStoppingMeeting ? "正在提炼..." : "结束会议(保持工时)"}
              </button>
              <button
                onClick={() => handleFinishMeeting(true)}
                disabled={isStoppingMeeting}
                className="px-2.5 py-1 bg-rose-600 hover:bg-rose-700 text-white rounded text-[11px] font-semibold transition-colors cursor-pointer flex items-center gap-1 disabled:opacity-50 shadow-2xs"
                title="结束整场会议并同时停止今日工时"
              >
                <Square className="w-3 h-3 fill-current" />
                {isStoppingMeeting ? "正在提炼..." : "结束会议并结工时"}
              </button>
            </div>
          </div>
        )}

        {/* 选项卡视图内容 */}
        {activeTab === "workspace" && (
          <div className="flex-1 flex min-h-0 p-5 gap-5 overflow-hidden">
            {/* 左侧：项目卡片侧栏列表 */}
            <div className="w-72 flex flex-col bg-white border border-slate-200/90 rounded-xl shadow-xs shrink-0 overflow-hidden">
              <div className="p-3.5 border-b border-slate-100 flex items-center justify-between bg-slate-50/50">
                <span className="text-xs font-bold text-slate-700 flex items-center gap-1.5">
                  <Layers className="w-3.5 h-3.5 text-slate-400" />
                  研发与会议项目
                </span>
                <span className="text-[11px] font-medium text-slate-400">{projects.length} 个项目</span>
              </div>

              <div className="flex-1 overflow-y-auto p-2.5 space-y-1.5">
                {projects.map((p) => {
                  const isSelected = selectedProjectId === p.id;
                  return (
                    <div
                      key={p.id}
                      onClick={() => setSelectedProjectId(p.id)}
                      className={`p-3 rounded-lg border transition-all cursor-pointer ${
                        isSelected
                          ? "bg-slate-900 border-slate-900 text-white shadow-xs"
                          : "bg-white hover:bg-slate-50 border-slate-100 text-slate-700"
                      }`}
                    >
                      <div className="flex items-center justify-between mb-1">
                        <div className="flex items-center gap-2 min-w-0">
                          <span
                            className={`w-2 h-2 rounded-full shrink-0 ${
                              p.is_running
                                ? "bg-emerald-400 animate-pulse"
                                : p.is_paused
                                ? "bg-amber-400"
                                : isSelected
                                ? "bg-slate-600"
                                : "bg-slate-300"
                            }`}
                          />
                          <span className={`text-xs font-semibold truncate ${isSelected ? "text-white" : "text-slate-800"}`}>
                            {p.name}
                          </span>
                        </div>
                        {p.is_running && (
                          <span className="px-1.5 py-0.2 rounded text-[10px] font-medium bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
                            计时中
                          </span>
                        )}
                      </div>

                      <div className="flex items-center justify-between text-[11px] opacity-80">
                        <span className="truncate max-w-[130px]">{p.client} · {p.entries_count}条</span>
                        <span className={`font-medium ${isSelected ? "text-indigo-300" : "text-indigo-600"}`}>
                          {p.today_duration !== "00:00:00" ? p.today_duration : ""}
                        </span>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>

            {/* 右侧：选中项目的工时明细看板 (一天一个卡片) */}
            <div className="flex-1 flex flex-col min-w-0 overflow-y-auto pr-1 space-y-4">
              {activeProject && (
                <>
                  {/* 项目主卡片 Header */}
                  <div className="bg-white border border-slate-200/80 rounded-xl p-5 shadow-sm space-y-3.5">
                    <div className="flex items-center justify-between gap-4">
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2.5 flex-wrap">
                          <h3 className="text-base font-bold text-slate-900 tracking-tight truncate">
                            {activeProject.name}
                          </h3>
                          <span className="px-2 py-0.5 rounded bg-slate-100 text-slate-600 text-xs font-medium whitespace-nowrap">
                            {activeProject.client}
                          </span>
                          <button
                            onClick={() => handleOpenEditProject(activeProject)}
                            className="px-2 py-0.5 rounded bg-indigo-50 hover:bg-indigo-100 border border-indigo-200/80 text-indigo-700 text-xs font-medium flex items-center gap-1 transition-all cursor-pointer shadow-2xs"
                            title="配置并修改项目的商业计费方式、价格单价、币种与舍入规则"
                          >
                            <Settings className="w-3 h-3 text-indigo-600" />
                            规则配置
                          </button>
                          <button
                            onClick={() => setProjectToDelete(activeProject)}
                            className="px-2 py-0.5 rounded bg-rose-50 hover:bg-rose-100 border border-rose-200/80 text-rose-700 text-xs font-medium flex items-center gap-1 transition-all cursor-pointer shadow-2xs"
                            title="删除项目及其关联记录"
                          >
                            <Trash2 className="w-3 h-3 text-rose-600" />
                            删除项目
                          </button>
                          {activeProject.is_running && (
                            <span className="px-2 py-0.5 rounded bg-emerald-50 text-emerald-700 text-xs font-semibold flex items-center gap-1.5 whitespace-nowrap">
                              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-ping" />
                              正在计时
                            </span>
                          )}
                        </div>
                        <div className="flex items-center gap-2 text-xs text-slate-500 mt-1.5 flex-wrap">
                          <span className="whitespace-nowrap">
                            标准: {hideAmount ? "****** (金额已隐藏)" : activeProject.rate_info}
                          </span>
                          <span className="text-slate-300">•</span>
                          <span className="whitespace-nowrap">
                            今日投入: <strong className="text-slate-800 font-semibold">{activeProject.today_duration}</strong> ({activeProject.today_days})
                          </span>
                        </div>
                      </div>

                      {/* 大时钟与控制按钮 */}
                      <div className="flex items-center gap-4 shrink-0">
                        <div
                          className={`text-2xl font-bold font-mono tracking-tight ${
                            activeProject.is_running ? "text-emerald-600" : "text-slate-700"
                          }`}
                        >
                          {activeProject.timer_text}
                        </div>

                        {!activeProject.is_running && !activeProject.is_paused && (
                          <div className="flex items-center gap-2">
                            <button
                              onClick={() => handleStart(activeProject.id)}
                              className="h-8 px-3.5 bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg text-xs font-semibold flex items-center gap-1.5 shadow-xs active:scale-95 transition-all cursor-pointer"
                              title={activeProject.today_duration !== "00:00:00" ? "恢复并继续今日已结工时（时长将累加）" : "启动工时计时"}
                            >
                              <Play className="w-3.5 h-3.5 fill-current" />
                              {activeProject.today_duration !== "00:00:00" ? "恢复/继续工时" : "开始工时"}
                            </button>
                            <button
                              onClick={() => {
                                setMeetingSelectedPid(activeProject.id);
                                setShowMeetingModal(true);
                              }}
                              className="h-8 px-3 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg text-xs font-medium flex items-center gap-1.5 shadow-xs active:scale-95 transition-all cursor-pointer"
                            >
                              <Mic className="w-3.5 h-3.5" />
                              发起会议
                            </button>
                            <button
                              onClick={() => setShowNoteModal(true)}
                              className="h-8 px-3 bg-slate-100 hover:bg-slate-200 border border-slate-200/80 text-slate-700 rounded-lg text-xs font-medium flex items-center gap-1.5 shadow-2xs active:scale-95 transition-all"
                              title="记录工作便签与备忘"
                            >
                              <StickyNote className="w-3.5 h-3.5 text-amber-500" />
                              便签
                            </button>
                          </div>
                        )}

                        {activeProject.is_running && (
                          <div className="flex items-center gap-2">
                            {globalStats.is_recording_audio &&
                            globalStats.active_recording_project_id === activeProject.id ? (
                              <button
                                onClick={handleStopMeeting}
                                disabled={isStoppingMeeting}
                                className="h-8 px-2.5 bg-rose-600 hover:bg-rose-700 text-white rounded-lg text-xs font-semibold flex items-center gap-1 shadow-xs transition-all disabled:opacity-50"
                                title="单独结束当前会议录音并提炼AI纪要（工时继续计时）"
                              >
                                <Square className="w-3.5 h-3.5 fill-current" />
                                {isStoppingMeeting ? "正在提炼..." : "结束会议"}
                              </button>
                            ) : (
                              <button
                                onClick={() => {
                                  setMeetingSelectedPid(activeProject.id);
                                  setShowMeetingModal(true);
                                }}
                                className="h-8 px-2.5 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg text-xs font-medium flex items-center gap-1 shadow-xs transition-all"
                                title="在此项目中召开会议并启动录音"
                              >
                                <Mic className="w-3.5 h-3.5" />
                                开会/录音
                              </button>
                            )}
                            <button
                              onClick={() => setShowNoteModal(true)}
                              className="h-8 px-2.5 bg-slate-100 hover:bg-slate-200 border border-slate-200 text-slate-700 rounded-lg text-xs font-medium flex items-center gap-1 shadow-2xs transition-all"
                            >
                              <StickyNote className="w-3.5 h-3.5 text-amber-500" />
                              便签
                            </button>
                            <button
                              onClick={() => handlePause(activeProject.id)}
                              className="h-8 px-3 bg-amber-500 hover:bg-amber-600 text-white rounded-lg text-xs font-medium flex items-center gap-1.5 shadow-xs transition-all"
                            >
                              <Pause className="w-3.5 h-3.5 fill-current" />
                              暂停
                            </button>
                            <button
                              onClick={() => handleStop(activeProject.id)}
                              className="h-8 px-3 bg-rose-600 hover:bg-rose-700 text-white rounded-lg text-xs font-medium flex items-center gap-1.5 shadow-xs transition-all"
                            >
                              <Square className="w-3.5 h-3.5 fill-current" />
                              结束工时
                            </button>
                          </div>
                        )}

                        {activeProject.is_paused && (
                          <div className="flex items-center gap-2">
                            <button
                              onClick={() => {
                                setMeetingSelectedPid(activeProject.id);
                                setShowMeetingModal(true);
                              }}
                              className="h-8 px-2.5 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg text-xs font-medium flex items-center gap-1 shadow-xs transition-all"
                              title="在此项目中召开会议并启动录音"
                            >
                              <Mic className="w-3.5 h-3.5" />
                              开会/录音
                            </button>
                            <button
                              onClick={() => setShowNoteModal(true)}
                              className="h-8 px-2.5 bg-slate-100 hover:bg-slate-200 border border-slate-200 text-slate-700 rounded-lg text-xs font-medium flex items-center gap-1 shadow-2xs transition-all"
                            >
                              <StickyNote className="w-3.5 h-3.5 text-amber-500" />
                              便签
                            </button>
                            <button
                              onClick={() => handleResume(activeProject.id)}
                              className="h-8 px-3.5 bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg text-xs font-medium flex items-center gap-1.5 shadow-xs transition-all"
                            >
                              <Play className="w-3.5 h-3.5 fill-current" />
                              继续工时
                            </button>
                            <button
                              onClick={() => handleStop(activeProject.id)}
                              className="h-8 px-3 bg-rose-600 hover:bg-rose-700 text-white rounded-lg text-xs font-medium flex items-center gap-1.5 shadow-xs transition-all"
                            >
                              <Square className="w-3.5 h-3.5 fill-current" />
                              结束工时
                            </button>
                          </div>
                        )}
                      </div>
                    </div>
                  </div>

                  {/* ---------------- 工时控制台与年月筛选栏 ---------------- */}
                  <div className="flex items-center justify-between bg-white border border-slate-200/80 rounded-xl px-4 py-2.5 shadow-sm">
                    {/* 年月控制器 */}
                    <div className="flex items-center gap-2">
                      <div className="flex items-center bg-slate-100 rounded-lg p-0.5 border border-slate-200/60">
                        <button
                          onClick={handlePrevMonth}
                          className="p-1 hover:bg-white rounded-md text-slate-600 transition-colors"
                          title="上个月"
                        >
                          <ChevronLeft className="w-3.5 h-3.5" />
                        </button>
                        <span className="px-2.5 text-xs font-bold text-slate-800 tracking-tight flex items-center gap-1.5">
                          <Calendar className="w-3.5 h-3.5 text-indigo-500" />
                          {selectedMonth}
                        </span>
                        <button
                          onClick={handleNextMonth}
                          className="p-1 hover:bg-white rounded-md text-slate-600 transition-colors"
                          title="下个月"
                        >
                          <ChevronRight className="w-3.5 h-3.5" />
                        </button>
                      </div>

                      <button
                        onClick={() => {
                          const n = new Date();
                          setSelectedMonth(`${n.getFullYear()}-${String(n.getMonth() + 1).padStart(2, "0")}`);
                        }}
                        className="px-2 py-1 text-[11px] font-medium text-slate-500 hover:text-indigo-600 hover:bg-slate-100 rounded-md transition-colors"
                      >
                        本月
                      </button>
                    </div>

                    {/* 视图模式切换与补录动作入口 */}
                    <div className="flex items-center gap-2">
                      {/* 视图切换：流水列表 vs 日历视图 */}
                      <div className="flex items-center bg-slate-100 rounded-lg p-0.5 border border-slate-200/60">
                        <button
                          onClick={() => setViewMode("list")}
                          className={`px-2.5 py-1 rounded-md text-xs font-medium flex items-center gap-1 transition-all ${
                            viewMode === "list"
                              ? "bg-white text-slate-900 shadow-2xs font-semibold"
                              : "text-slate-500 hover:text-slate-900"
                          }`}
                        >
                          <List className="w-3 h-3" />
                          流水
                        </button>
                        <button
                          onClick={() => setViewMode("calendar")}
                          className={`px-2.5 py-1 rounded-md text-xs font-medium flex items-center gap-1 transition-all ${
                            viewMode === "calendar"
                              ? "bg-white text-slate-900 shadow-2xs font-semibold"
                              : "text-slate-500 hover:text-slate-900"
                          }`}
                        >
                          <CalendarDays className="w-3 h-3" />
                          日历
                        </button>
                      </div>

                      <div className="h-4 w-px bg-slate-200 mx-1" />

                      {/* 补录工时入口 */}
                      <button
                        onClick={() => setShowManualEntryModal(true)}
                        className="h-7 px-2.5 rounded-lg border border-slate-200 bg-white hover:bg-slate-50 text-slate-700 text-xs font-medium flex items-center gap-1 shadow-2xs transition-all cursor-pointer"
                        title="单日补录工时投入"
                      >
                        <PlusCircle className="w-3.5 h-3.5 text-indigo-500" />
                        补录工时
                      </button>

                      {/* 智能批量补录入口 */}
                      <button
                        onClick={() => setShowBatchModal(true)}
                        className="h-7 px-2.5 rounded-lg border border-indigo-300 bg-indigo-50 hover:bg-indigo-100 text-indigo-700 text-xs font-bold flex items-center gap-1 shadow-2xs transition-all cursor-pointer"
                        title="按起止时间段自动检索 Git 或粘贴 AI 智能体工作总结批量智能入库"
                      >
                        <Sparkles className="w-3.5 h-3.5 text-indigo-600" />
                        智能批量补录
                      </button>

                      {/* 补录会议+上传录音入口 */}
                      <button
                        onClick={() => {
                          setManualMeetingPid(activeProject.id);
                          setShowManualMeetingModal(true);
                        }}
                        className="h-7 px-2.5 rounded-lg border border-slate-200 bg-white hover:bg-slate-50 text-slate-700 text-xs font-medium flex items-center gap-1 shadow-2xs transition-all cursor-pointer"
                      >
                        <Upload className="w-3.5 h-3.5 text-indigo-600" />
                        补录会议/录音
                      </button>
                    </div>
                  </div>

                  {/* ---------------- 模式 A：流水列表视图 ---------------- */}
                  {viewMode === "list" && (
                    <div className="space-y-3.5">
                      {dayGroups.length === 0 ? (
                        <div className="bg-white border border-slate-200/90 rounded-xl p-8 text-center space-y-2">
                          <p className="text-xs text-slate-400 font-medium">
                            【{selectedMonth}】该月份暂无工时记录
                          </p>
                          <button
                            onClick={() => setShowManualEntryModal(true)}
                            className="text-xs text-indigo-600 hover:underline font-semibold"
                          >
                            + 点击补录该月工时
                          </button>
                        </div>
                      ) : (
                        dayGroups.map((group) => (
                          <div
                            key={group.date_str}
                            className="bg-white border border-slate-200/80 rounded-xl p-4.5 shadow-sm hover:shadow-md transition-shadow space-y-3"
                          >
                            {/* 每日卡片 Header */}
                            <div className="flex items-center justify-between border-b border-slate-100 pb-2.5">
                              <div className="flex items-center gap-2.5">
                                <span className="text-sm font-bold text-slate-900 flex items-center gap-1.5">
                                  📅 {group.display_date}
                                </span>
                                <span className="px-2 py-0.5 rounded bg-slate-100 text-slate-600 text-[11px] font-medium">
                                  {group.entries_count} 笔工时记录
                                </span>
                              </div>

                              <div className="flex items-center gap-3.5 text-xs">
                                <span className="text-slate-500">投入: <strong className="text-slate-700 font-medium">{group.total_duration}</strong></span>
                                <span className="text-slate-500">折合: <strong className="text-slate-700 font-medium">{group.total_days}</strong></span>
                                {!hideAmount && (
                                  <span className="text-slate-500">价值: <strong className="text-emerald-600 font-semibold">{group.total_amount}</strong></span>
                                )}
                              </div>
                            </div>

                            {/* 当日明细项列表 */}
                            <div className="space-y-2.5">
                              {group.entries.map((entry) => {
                                const isLiveRecording = entry.status === "recording";
                                const isSummarizing = entry.status === "summarizing";

                                return (
                                <div
                                  key={entry.id}
                                  className={`p-3.5 rounded-lg border shadow-2xs space-y-2 transition-all ${
                                    isLiveRecording
                                      ? "bg-rose-50/70 border-rose-200/80 ring-1 ring-rose-300/60"
                                      : isSummarizing
                                      ? "bg-amber-50/70 border-amber-200/80 ring-1 ring-amber-300/60"
                                      : "bg-slate-50/70 border-slate-200/60 hover:bg-slate-50/90"
                                  }`}
                                >
                                  <div className="flex items-center justify-between text-xs">
                                    <div className="flex items-center gap-3 text-slate-600">
                                      <span className="font-semibold text-slate-900 flex items-center gap-1">
                                        <Clock className="w-3.5 h-3.5 text-slate-400" />
                                        {entry.time_range}
                                      </span>
                                      <span>投入: {entry.duration_text}</span>
                                      <span>折算: {entry.days_text}</span>
                                      {!hideAmount && (
                                        <span className="text-emerald-600 font-semibold">{entry.amount_text}</span>
                                      )}
                                    </div>

                                    {/* 状态与结算徽章区 */}
                                    <div className="flex items-center gap-2">
                                      {isLiveRecording && (
                                        <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold bg-rose-100 text-rose-700 border border-rose-300">
                                          <span className="w-1.5 h-1.5 rounded-full bg-rose-600 animate-ping" />
                                          <span>会议录音中</span>
                                        </div>
                                      )}
                                      {isSummarizing && (
                                        <div className="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold bg-amber-100 text-amber-800 border border-amber-300">
                                          <RefreshCw className="w-3 h-3 text-amber-600 animate-spin" />
                                          <span>AI提炼中</span>
                                        </div>
                                      )}

                                      {/* 结算状态标签 (可点击直接切换) */}
                                      {!isLiveRecording && (
                                        <button
                                          onClick={() => handleToggleEntryInvoiced(entry.id, entry.is_invoiced)}
                                          title={entry.is_invoiced ? "点击重置为待结算" : "点击标记为已结算对账"}
                                          className={`px-2 py-0.5 rounded-full text-[10px] font-bold flex items-center gap-1 border transition-all active:scale-95 ${
                                            entry.is_invoiced
                                              ? "bg-emerald-50 text-emerald-700 border-emerald-300 hover:bg-emerald-100"
                                              : "bg-amber-50 text-amber-700 border-amber-300 hover:bg-amber-100"
                                          }`}
                                        >
                                          {entry.is_invoiced ? (
                                            <>
                                              <CheckCheck className="w-3 h-3 text-emerald-600" />
                                              已结算
                                            </>
                                          ) : (
                                            <>
                                              <span className="w-1.5 h-1.5 rounded-full bg-amber-500 animate-pulse" />
                                              待结算
                                            </>
                                          )}
                                        </button>
                                      )}
                                    </div>
                                  </div>

                                  {(() => {
                                    const { summary: displaySummary, transcript: rawTranscript } = splitSummaryAndTranscript(entry.summary_text);
                                    const isExpanded = !!expandedSummaryEntryIds[entry.id];
                                    const isLongContent = displaySummary.length > 160 || displaySummary.split("\n").length > 4;
                                    return (
                                      <>
                                        <div className="relative">
                                          <div
                                            className={`markdown-body text-xs text-slate-700 leading-relaxed font-normal transition-all duration-200 ${
                                              !isExpanded && isLongContent
                                                ? "max-h-24 overflow-hidden mask-gradient-b"
                                                : ""
                                            }`}
                                            dangerouslySetInnerHTML={{ __html: renderMarkdown(displaySummary) }}
                                          />
                                          {isLongContent && (
                                            <div className="pt-1.5 flex items-center">
                                              <button
                                                type="button"
                                                onClick={() => toggleSummaryExpand(entry.id)}
                                                className="text-[11px] font-bold text-indigo-600 hover:text-indigo-800 flex items-center gap-1 cursor-pointer py-0.5 px-1.5 rounded hover:bg-indigo-50/80 transition-colors"
                                              >
                                                {isExpanded ? (
                                                  <>
                                                    <ChevronUp className="w-3.5 h-3.5" />
                                                    收起内容
                                                  </>
                                                ) : (
                                                  <>
                                                    <ChevronDown className="w-3.5 h-3.5" />
                                                    展开查看完整内容 ({displaySummary.length}字)
                                                  </>
                                                )}
                                              </button>
                                            </div>
                                          )}
                                          {/* 若该日包含会议，分层结构化展示会议与录音片段列表 */}
                                          {entry.meetings && entry.meetings.length > 0 && (
                                            <div className="mt-2.5 pt-2 border-t border-slate-100 space-y-2">
                                              <div className="text-[11px] font-bold text-slate-500 flex items-center gap-1.5">
                                                <Mic className="w-3.5 h-3.5 text-indigo-500" />
                                                <span>今日会议清单 ({entry.meetings.length} 场)</span>
                                              </div>
                                              <div className="space-y-1.5">
                                                {entry.meetings.map((m) => (
                                                  <div key={m.id} className="p-2.5 bg-slate-50/80 hover:bg-slate-50 rounded-lg border border-slate-200/80 text-xs">
                                                    <div className="flex items-center justify-between font-semibold text-slate-800">
                                                      <div className="flex items-center gap-1.5">
                                                        <span className="w-1.5 h-1.5 rounded-full bg-indigo-500" />
                                                        <span>{m.topic}</span>
                                                        <span className="text-[10px] text-slate-400 font-mono">({m.start_time}{m.end_time ? ` - ${m.end_time}` : ""} · {m.duration_text})</span>
                                                      </div>
                                                      <div className="flex items-center gap-1">
                                                        {m.status === "in_progress" && (
                                                          <button
                                                            onClick={() => handleStartClipForMeeting(m.id)}
                                                            className="px-1.5 py-0.5 bg-indigo-50 text-indigo-700 hover:bg-indigo-100 rounded text-[10px] font-bold cursor-pointer"
                                                            title="为此会议录制一段新的录音"
                                                          >
                                                            + 追加录音
                                                          </button>
                                                        )}
                                                      </div>
                                                    </div>

                                                    {/* 会议录音片段列表 */}
                                                    {m.clips && m.clips.length > 0 && (
                                                      <div className="mt-2 flex items-center gap-1.5 flex-wrap">
                                                        <span className="text-[10px] text-slate-400">录音片段:</span>
                                                        {m.clips.map((clip) => {
                                                          const isRetrying = retryingClipIds.has(clip.id);
                                                          const isPending = isRetrying || clip.status === "pending";
                                                          const isFailed = !isPending && (clip.status === "failed" || (!clip.transcript && clip.status !== "transcribed"));
                                                          const isTranscribed = !isPending && (clip.status === "transcribed" || !!clip.transcript);

                                                          return (
                                                            <div
                                                              key={clip.id}
                                                              className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-[10px] border transition-all ${
                                                                isFailed
                                                                  ? "bg-rose-50/80 border-rose-200 text-rose-700 shadow-2xs"
                                                                  : isPending
                                                                  ? "bg-amber-50/80 border-amber-200 text-amber-700 animate-pulse"
                                                                  : "bg-white border-slate-200 text-slate-700"
                                                              }`}
                                                            >
                                                              {/* 播放录音片段按钮 */}
                                                              <button
                                                                onClick={() => handleOpenAudioPlayer(clip.audio_path, `片段 ${clip.clip_order}`)}
                                                                className="font-semibold flex items-center gap-0.5 hover:underline cursor-pointer"
                                                                title="播放该录音片段"
                                                              >
                                                                <Volume2 className="w-2.5 h-2.5" />
                                                                <span>片段 {clip.clip_order} ({clip.duration_text})</span>
                                                              </button>

                                                              {/* 转写就绪状态：文稿查看与轻量重新转写 */}
                                                              {isTranscribed && (
                                                                <div className="flex items-center gap-1 border-l border-slate-200 pl-1.5 ml-0.5">
                                                                  {clip.transcript && (
                                                                    <button
                                                                      onClick={() => setActiveTranscriptModal({
                                                                        title: `🎙️ ${m.topic} - 片段 ${clip.clip_order} 转写稿`,
                                                                        transcript: clip.transcript!,
                                                                      })}
                                                                      className="text-indigo-600 hover:text-indigo-800 cursor-pointer flex items-center gap-0.5 font-medium"
                                                                      title="查看该片段转写文稿"
                                                                    >
                                                                      <FileText className="w-2.5 h-2.5" />
                                                                      <span>文稿</span>
                                                                    </button>
                                                                  )}
                                                                  <button
                                                                    onClick={() => handleRetryTranscribeClip(clip.id)}
                                                                    className="text-slate-400 hover:text-indigo-600 cursor-pointer p-0.5"
                                                                    title="重新转写此片段"
                                                                  >
                                                                    <RotateCw className="w-2.5 h-2.5" />
                                                                  </button>
                                                                </div>
                                                              )}

                                                              {/* 转写中状态 */}
                                                              {isPending && (
                                                                <span className="flex items-center gap-1 text-amber-600 font-medium border-l border-amber-200 pl-1.5 ml-0.5">
                                                                  <Loader2 className="w-2.5 h-2.5 animate-spin" />
                                                                  <span>转写中...</span>
                                                                </span>
                                                              )}

                                                              {/* 转写失败状态：醒目标识 + 再次转写按钮 */}
                                                              {isFailed && (
                                                                <div className="flex items-center gap-1 border-l border-rose-200 pl-1.5 ml-0.5">
                                                                  <span className="flex items-center gap-0.5 text-rose-600 font-semibold">
                                                                    <AlertCircle className="w-2.5 h-2.5" />
                                                                    <span>转写失败</span>
                                                                  </span>
                                                                  <button
                                                                    onClick={() => handleRetryTranscribeClip(clip.id)}
                                                                    className="px-1.5 py-0.5 rounded bg-rose-600 hover:bg-rose-700 text-white font-bold text-[9px] flex items-center gap-0.5 cursor-pointer shadow-2xs active:scale-95 transition-all"
                                                                    title="重试转写此录音片段"
                                                                  >
                                                                    <RotateCw className="w-2.5 h-2.5" />
                                                                    <span>再次转写</span>
                                                                  </button>
                                                                </div>
                                                              )}
                                                            </div>
                                                          );
                                                        })}
                                                      </div>
                                                    )}
                                                  </div>
                                                ))}
                                              </div>
                                            </div>
                                          )}
                                        </div>

                                        {/* 操作按钮区 */}
                                        <div className="flex items-center justify-between pt-1">
                                          <div className="flex items-center gap-2">
                                            {isLiveRecording && (
                                              <button
                                                onClick={handleStopMeeting}
                                                disabled={isStoppingMeeting}
                                                className="h-6.5 px-2.5 rounded bg-rose-600 hover:bg-rose-700 text-white text-[11px] font-bold flex items-center gap-1 transition-all cursor-pointer shadow-2xs"
                                                title="立即结束本次会议并保存录音"
                                              >
                                                <Square className="w-3 h-3 fill-current" />
                                                结束会议并提炼纪要
                                              </button>
                                            )}
                                            {entry.has_audio && (
                                              <button
                                                onClick={() => handleOpenAudioPlayer(entry.audio_path, `${entry.date_str} 会议录音凭证`)}
                                                className="h-6.5 px-2.5 rounded bg-indigo-50 border border-indigo-200 hover:bg-indigo-100 text-indigo-700 text-[11px] font-semibold flex items-center gap-1 transition-all cursor-pointer shadow-2xs"
                                                title="点击打开内置播放器，支持进度拖拽、快进后退与完整路径复制"
                                              >
                                                <Volume2 className="w-3.5 h-3.5 text-indigo-600" />
                                                播放录音
                                              </button>
                                            )}
                                            {rawTranscript && (
                                              <button
                                                onClick={() => setActiveTranscriptModal({
                                                  title: `📅 ${entry.date_str} 会议全量转写实录`,
                                                  transcript: rawTranscript,
                                                })}
                                                className="h-6.5 px-2.5 rounded bg-slate-100 hover:bg-slate-200 text-slate-700 text-[11px] font-medium flex items-center gap-1 transition-all cursor-pointer shadow-2xs"
                                                title="在独立窗口中查看完整数万字逐字转写文稿"
                                              >
                                                <FileText className="w-3 h-3 text-slate-500" />
                                                查看转写实录 ({rawTranscript.length}字)
                                              </button>
                                            )}
                                            {entry.has_audio && !isLiveRecording && (
                                              <button
                                                onClick={() => handleTranscribe(entry)}
                                                className="h-6.5 px-2.5 rounded bg-amber-50 border border-amber-200 text-amber-700 hover:bg-amber-100 text-[11px] font-medium flex items-center gap-1 transition-all cursor-pointer shadow-2xs"
                                                title="使用当前配置的 AI 重新识别音频并提炼纪要"
                                              >
                                                <Sparkles className="w-3 h-3 text-amber-600" />
                                                重新提炼
                                              </button>
                                            )}
                                          </div>

                                          <div className="flex items-center gap-2">
                                            {!isLiveRecording && (
                                              <>
                                                <button
                                                  onClick={() => {
                                                    navigator.clipboard.writeText(displaySummary);
                                                    showToast("📋 会议纪要已复制到剪贴板！可直接发送给 AI 智能体");
                                                  }}
                                                  className="h-6.5 px-2.5 rounded bg-slate-100 hover:bg-slate-200 text-slate-700 text-[11px] font-semibold flex items-center gap-1 shadow-2xs transition-all cursor-pointer"
                                                  title="一键复制精炼会议纪要，方便丢给 Agent 进行开发拆解"
                                                >
                                                  <Copy className="w-3 h-3 text-slate-500" />
                                                  复制纪要
                                                </button>
                                                <button
                                                  onClick={() => setActiveEditSummaryModal({
                                                    entry_id: entry.id,
                                                    title: `📅 ${entry.date_str} 会议纪要与交付说明`,
                                                    project_name: entry.project_name || activeProject?.name || "项目交付",
                                                    date_str: entry.date_str,
                                                    summary: displaySummary,
                                                    raw_transcript: rawTranscript,
                                                  })}
                                                  className="h-6.5 px-2.5 rounded bg-indigo-50 border border-indigo-200 hover:bg-indigo-100 text-indigo-700 text-[11px] font-bold flex items-center gap-1 shadow-2xs transition-all cursor-pointer"
                                                  title="修改与微调 AI 提取的要点，保存后交付 Agent 实施开发"
                                                >
                                                  <Edit3 className="w-3 h-3 text-indigo-600" />
                                                  编辑纪要
                                                </button>
                                                <button
                                                  disabled={isExportingPdf}
                                                  onClick={() => handleExportPdf({
                                                    title: `【${entry.project_name || activeProject?.name || "项目"}】会议纪要与技术决议`,
                                                    projectName: entry.project_name || activeProject?.name || "技术项目",
                                                    dateStr: entry.date_str,
                                                    summaryContent: displaySummary,
                                                  })}
                                                  className="h-6.5 px-2.5 rounded bg-emerald-50 border border-emerald-200 hover:bg-emerald-100 text-emerald-700 text-[11px] font-bold flex items-center gap-1 shadow-2xs transition-all cursor-pointer disabled:opacity-50"
                                                  title="导出为精美商务 PDF 文档，可直接交付客户确认"
                                                >
                                                  <FileDown className="w-3 h-3 text-emerald-600" />
                                                  导出PDF
                                                </button>
                                              </>
                                            )}
                                            <button
                                              onClick={() => {
                                                setSelectedEntry(entry);
                                                setShowDetailModal(true);
                                              }}
                                              className="h-6.5 px-2.5 rounded bg-white border border-slate-200 text-slate-600 hover:bg-slate-50 text-[11px] font-medium flex items-center gap-1 shadow-2xs cursor-pointer"
                                            >
                                              <FileText className="w-3 h-3 text-slate-400" />
                                              查看详情
                                            </button>
                                          </div>
                                        </div>
                                      </>
                                    );
                                  })()}
                                </div>
                                );
                              })}
                            </div>
                          </div>
                        ))
                      )}
                    </div>
                  )}

                  {/* ---------------- 模式 B：月历视图 (Calendar View) ---------------- */}
                  {viewMode === "calendar" && (() => {
                    const [yearNum, monthNum] = selectedMonth.split("-").map(Number);
                    const firstDayOfWeek = new Date(yearNum, monthNum - 1, 1).getDay(); // 0 is Sunday
                    const totalDaysInMonth = new Date(yearNum, monthNum, 0).getDate();
                    // 周一到周日偏移 (0->6, 1->0, 2->1 ...)
                    const offset = (firstDayOfWeek + 6) % 7;
                    const weekHeaders = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
                    
                    const groupDateMap = new Map(dayGroups.map((g) => [g.date_str, g]));
                    const selectedDayGroup = calendarSelectedDate ? groupDateMap.get(calendarSelectedDate) : null;

                    return (
                      <div className="space-y-4">
                        <div className="bg-white border border-slate-200/90 rounded-xl p-4 shadow-xs">
                          {/* 星期表头 */}
                          <div className="grid grid-cols-7 gap-2 mb-2 text-center text-xs font-bold text-slate-500 pb-2 border-b border-slate-100">
                            {weekHeaders.map((w, idx) => (
                              <div key={w} className={idx >= 5 ? "text-amber-600" : ""}>{w}</div>
                            ))}
                          </div>

                          {/* 月历日期网格 */}
                          <div className="grid grid-cols-7 gap-2">
                            {/* 前置留白 */}
                            {Array.from({ length: offset }).map((_, i) => (
                              <div key={`empty-${i}`} className="h-24 rounded-lg bg-slate-50/40 border border-transparent" />
                            ))}

                            {/* 本月各天 */}
                            {Array.from({ length: totalDaysInMonth }).map((_, i) => {
                              const dayNum = i + 1;
                              const dStr = `${yearNum}-${String(monthNum).padStart(2, "0")}-${String(dayNum).padStart(2, "0")}`;
                              const group = groupDateMap.get(dStr);
                              const isToday = dStr === new Date().toISOString().split("T")[0];
                              const isSelected = calendarSelectedDate === dStr;
                              const hasAudio = group?.entries.some((e) => e.has_audio);

                              return (
                                <div
                                  key={dStr}
                                  onClick={() => setCalendarSelectedDate(dStr === calendarSelectedDate ? null : dStr)}
                                  className={`h-24 p-2 rounded-lg border transition-all cursor-pointer flex flex-col justify-between ${
                                    isSelected
                                      ? "ring-2 ring-indigo-500 border-indigo-500 bg-indigo-50/40"
                                      : group
                                      ? "bg-white hover:bg-slate-50 border-slate-200/90 shadow-2xs"
                                      : "bg-slate-50/60 hover:bg-slate-100/60 border-slate-100 text-slate-400"
                                  }`}
                                >
                                  <div className="flex items-center justify-between">
                                    <span
                                      className={`text-xs font-bold w-5 h-5 flex items-center justify-center rounded-full ${
                                        isToday
                                          ? "bg-indigo-600 text-white"
                                          : group
                                          ? "text-slate-800"
                                          : "text-slate-400"
                                      }`}
                                    >
                                      {dayNum}
                                    </span>
                                    {hasAudio && (
                                      <span title="包含会议/录音凭证">
                                        <Volume2 className="w-3 h-3 text-indigo-500" />
                                      </span>
                                    )}
                                  </div>

                                  {group ? (
                                    <div className="space-y-0.5 text-left">
                                      <div className="text-[11px] font-bold text-slate-900 leading-tight">
                                        {group.total_duration}
                                      </div>
                                      <div className="text-[10px] text-slate-500">
                                        {group.total_days}
                                      </div>
                                      {!hideAmount && (
                                        <div className="text-[10px] font-semibold text-emerald-600 truncate">
                                          {group.total_amount}
                                        </div>
                                      )}
                                    </div>
                                  ) : (
                                    <div className="text-[10px] text-slate-300 text-center py-2">
                                      无工时
                                    </div>
                                  )}
                                </div>
                              );
                            })}
                          </div>
                        </div>

                        {/* 点击日历某一天时展开的流水明细抽屉卡片 */}
                        {calendarSelectedDate && (
                          <div className="bg-white border border-slate-200/90 rounded-xl p-4 shadow-xs space-y-3 animate-in fade-in duration-150">
                            <div className="flex items-center justify-between border-b border-slate-100 pb-2.5">
                              <div className="flex items-center gap-2">
                                <span className="text-sm font-bold text-slate-900">
                                  📅 {calendarSelectedDate} 工时明细
                                </span>
                                {selectedDayGroup && (
                                  <span className="text-xs text-slate-500">
                                    （共 {selectedDayGroup.entries_count} 条 · {selectedDayGroup.total_duration}{!hideAmount ? ` · ${selectedDayGroup.total_amount}` : ""}）
                                  </span>
                                )}
                              </div>
                              <button
                                onClick={() => setCalendarSelectedDate(null)}
                                className="text-xs text-slate-400 hover:text-slate-700"
                              >
                                ✕ 收起
                              </button>
                            </div>

                            {!selectedDayGroup || selectedDayGroup.entries.length === 0 ? (
                              <div className="p-4 text-center text-xs text-slate-400">
                                当日暂无工时流水，
                                <button
                                  onClick={() => {
                                    setManualEntryDate(calendarSelectedDate);
                                    setShowManualEntryModal(true);
                                  }}
                                  className="text-indigo-600 font-semibold hover:underline ml-1"
                                >
                                  + 立即补录
                                </button>
                              </div>
                            ) : (
                              <div className="space-y-2">
                                {selectedDayGroup.entries.map((entry) => (
                                  <div
                                    key={entry.id}
                                    className="p-3 rounded-lg bg-slate-50/60 border border-slate-100 space-y-1.5"
                                  >
                                    <div className="flex items-center justify-between text-xs">
                                      <div className="flex items-center gap-3 text-slate-600">
                                        <span className="font-semibold text-slate-900 flex items-center gap-1">
                                          <Clock className="w-3.5 h-3.5 text-slate-400" />
                                          {entry.time_range}
                                        </span>
                                        <span>投入: {entry.duration_text}</span>
                                        <span>折算: {entry.days_text}</span>
                                        {!hideAmount && (
                                          <span className="text-emerald-600 font-semibold">{entry.amount_text}</span>
                                        )}
                                      </div>

                                      <button
                                        onClick={() => handleToggleEntryInvoiced(entry.id, entry.is_invoiced)}
                                        title={entry.is_invoiced ? "点击重置为待结算" : "点击标记为已结算对账"}
                                        className={`px-2 py-0.5 rounded-full text-[10px] font-bold flex items-center gap-1 border transition-all active:scale-95 ${
                                          entry.is_invoiced
                                            ? "bg-emerald-50 text-emerald-700 border-emerald-300 hover:bg-emerald-100"
                                            : "bg-amber-50 text-amber-700 border-amber-300 hover:bg-amber-100"
                                        }`}
                                      >
                                        {entry.is_invoiced ? "已结算" : "待结算"}
                                      </button>
                                    </div>
                                    {(() => {
                                      const { summary: displaySummary, transcript: rawTranscript } = splitSummaryAndTranscript(entry.summary_text);
                                      const isExpanded = !!expandedSummaryEntryIds[entry.id];
                                      const isLongContent = displaySummary.length > 160 || displaySummary.split("\n").length > 4;
                                      return (
                                        <>
                                          <div className="relative">
                                            <div
                                              className={`markdown-body text-xs text-slate-700 leading-relaxed font-normal transition-all duration-200 ${
                                                !isExpanded && isLongContent
                                                  ? "max-h-24 overflow-hidden mask-gradient-b"
                                                  : ""
                                              }`}
                                              dangerouslySetInnerHTML={{ __html: renderMarkdown(displaySummary) }}
                                            />
                                            {isLongContent && (
                                              <div className="pt-1.5 flex items-center">
                                                <button
                                                  type="button"
                                                  onClick={() => toggleSummaryExpand(entry.id)}
                                                  className="text-[11px] font-bold text-indigo-600 hover:text-indigo-800 flex items-center gap-1 cursor-pointer py-0.5 px-1.5 rounded hover:bg-indigo-50/80 transition-colors"
                                                >
                                                  {isExpanded ? (
                                                    <>
                                                      <ChevronUp className="w-3.5 h-3.5" />
                                                      收起内容
                                                    </>
                                                  ) : (
                                                    <>
                                                      <ChevronDown className="w-3.5 h-3.5" />
                                                      展开查看完整内容 ({displaySummary.length}字)
                                                    </>
                                                  )}
                                                </button>
                                              </div>
                                            )}
                                          </div>
                                          <div className="flex items-center gap-2 pt-1">
                                            {entry.has_audio && (
                                              <button
                                                onClick={() => handleOpenAudioPlayer(entry.audio_path, `${entry.date_str} 会议录音凭证`)}
                                                className="h-6 px-2.5 rounded bg-indigo-50 border border-indigo-200 hover:bg-indigo-100 text-indigo-700 text-[11px] font-semibold flex items-center gap-1 transition-all cursor-pointer shadow-2xs"
                                                title="点击打开内置播放器，支持进度拖拽、快进后退与完整路径复制"
                                              >
                                                <Volume2 className="w-3 h-3 text-indigo-600" />
                                                播放录音
                                              </button>
                                            )}
                                            {rawTranscript && (
                                              <button
                                                onClick={() => setActiveTranscriptModal({
                                                  title: `📅 ${entry.date_str} 会议全量转写实录`,
                                                  transcript: rawTranscript,
                                                })}
                                                className="h-6 px-2.5 rounded bg-slate-100 hover:bg-slate-200 text-slate-700 text-[11px] font-medium flex items-center gap-1 transition-all cursor-pointer shadow-2xs"
                                                title="在独立窗口中查看完整数万字逐字转写文稿"
                                              >
                                                <FileText className="w-3 h-3 text-slate-500" />
                                                查看转写实录 ({rawTranscript.length}字)
                                              </button>
                                            )}
                                            <button
                                              onClick={() => {
                                                navigator.clipboard.writeText(displaySummary);
                                                showToast("📋 会议纪要已复制到剪贴板！可直接发送给 AI 智能体");
                                              }}
                                              className="h-6 px-2 rounded bg-slate-100 hover:bg-slate-200 text-slate-700 text-[11px] font-semibold flex items-center gap-1 shadow-2xs transition-all cursor-pointer"
                                              title="一键复制精炼会议纪要"
                                            >
                                              <Copy className="w-3 h-3 text-slate-500" />
                                              复制纪要
                                            </button>
                                            <button
                                              onClick={() => setActiveEditSummaryModal({
                                                entry_id: entry.id,
                                                title: `📅 ${entry.date_str} 会议纪要与交付说明`,
                                                project_name: entry.project_name || activeProject?.name || "项目交付",
                                                date_str: entry.date_str,
                                                summary: displaySummary,
                                                raw_transcript: rawTranscript,
                                              })}
                                              className="h-6 px-2 rounded bg-indigo-50 border border-indigo-200 hover:bg-indigo-100 text-indigo-700 text-[11px] font-bold flex items-center gap-1 shadow-2xs transition-all cursor-pointer"
                                              title="编辑修改会议纪要"
                                            >
                                              <Edit3 className="w-3 h-3 text-indigo-600" />
                                              编辑纪要
                                            </button>
                                            <button
                                              disabled={isExportingPdf}
                                              onClick={() => handleExportPdf({
                                                title: `【${entry.project_name || activeProject?.name || "项目"}】会议纪要与技术决议`,
                                                projectName: entry.project_name || activeProject?.name || "技术项目",
                                                dateStr: entry.date_str,
                                                summaryContent: displaySummary,
                                              })}
                                              className="h-6 px-2 rounded bg-emerald-50 border border-emerald-200 hover:bg-emerald-100 text-emerald-700 text-[11px] font-bold flex items-center gap-1 shadow-2xs transition-all cursor-pointer disabled:opacity-50"
                                              title="导出为精美商务 PDF 文档，可直接交付客户确认"
                                            >
                                              <FileDown className="w-3 h-3 text-emerald-600" />
                                              导出PDF
                                            </button>
                                          </div>
                                        </>
                                      );
                                    })()}
                                  </div>
                                ))}
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    );
                  })()}
                </>
              )}
            </div>
          </div>
        )}

        {/* 选项卡视图：统计报表与多维对账结算导出 (重构：清爽列表 + 紧凑条件栏 + 弹窗配置) */}
        {activeTab === "stats" && (
          <div className="flex-1 p-6 overflow-y-auto space-y-4">
            {/* 1. 顶部指标概览栏 */}
            <div className={`grid gap-4 ${hideAmount ? "grid-cols-2" : "grid-cols-4"}`}>
              <div className="bg-white border border-slate-200/80 rounded-xl p-4 shadow-2xs">
                <div className="text-[11px] font-medium text-slate-400">核算总投入工时</div>
                <div className="text-xl font-extrabold text-slate-900 mt-1">
                  {settlementReport ? settlementReport.total_days.toFixed(2) : "0.00"}{" "}
                  <span className="text-xs font-normal text-slate-500">人天</span>
                </div>
              </div>
              <div className="bg-white border border-slate-200/80 rounded-xl p-4 shadow-2xs">
                <div className="text-[11px] font-medium text-slate-400">核算对账记录条数</div>
                <div className="text-xl font-extrabold text-indigo-600 mt-1">
                  {settlementReport ? settlementReport.entries.length : 0}{" "}
                  <span className="text-xs font-normal text-slate-500">笔工时</span>
                </div>
              </div>
              {!hideAmount && (
                <>
                  <div className="bg-white border border-slate-200/80 rounded-xl p-4 shadow-2xs">
                    <div className="text-[11px] font-medium text-slate-400">总核算金额</div>
                    <div className="text-xl font-extrabold text-indigo-600 mt-1">
                      ¥ {settlementReport ? settlementReport.total_amount.toFixed(2) : "0.00"}
                    </div>
                  </div>
                  <div className="bg-white border border-emerald-200/80 bg-emerald-50/15 rounded-xl p-4 shadow-2xs">
                    <div className="text-[11px] font-medium text-emerald-600">已结算 / 已回款</div>
                    <div className="text-xl font-extrabold text-emerald-600 mt-1">
                      ¥ {settlementReport ? settlementReport.settled_amount.toFixed(2) : "0.00"}
                    </div>
                  </div>
                </>
              )}
            </div>

            {/* 2. 紧凑单行查询条件与操作栏 */}
            <div className="bg-white border border-slate-200/80 rounded-xl px-4 py-3 shadow-2xs flex items-center justify-between gap-4">
              <div className="flex items-center gap-3">
                {/* 状态快捷切换 */}
                <div className="flex items-center bg-slate-100 rounded-lg p-0.5 border border-slate-200/60 text-xs font-medium">
                  <button
                    onClick={() => {
                      setReportStatusFilter("all");
                      setTimeout(fetchSettlementReport, 50);
                    }}
                    className={`px-3 py-1 rounded-md transition-all ${
                      reportStatusFilter === "all" ? "bg-white text-slate-900 font-bold shadow-2xs" : "text-slate-500 hover:text-slate-800"
                    }`}
                  >
                    全部状态
                  </button>
                  <button
                    onClick={() => {
                      setReportStatusFilter("pending");
                      setTimeout(fetchSettlementReport, 50);
                    }}
                    className={`px-3 py-1 rounded-md transition-all ${
                      reportStatusFilter === "pending" ? "bg-amber-500 text-white font-bold shadow-2xs" : "text-slate-500 hover:text-slate-800"
                    }`}
                  >
                    待结算
                  </button>
                  <button
                    onClick={() => {
                      setReportStatusFilter("settled");
                      setTimeout(fetchSettlementReport, 50);
                    }}
                    className={`px-3 py-1 rounded-md transition-all ${
                      reportStatusFilter === "settled" ? "bg-emerald-600 text-white font-bold shadow-2xs" : "text-slate-500 hover:text-slate-800"
                    }`}
                  >
                    已结算
                  </button>
                </div>

                <div className="h-4 w-px bg-slate-200" />

                {/* 筛选摘要标签与弹窗触发器 */}
                <button
                  onClick={() => setShowFilterModal(true)}
                  className="h-8 px-3 rounded-lg border border-slate-200 hover:bg-slate-50 text-xs font-medium text-slate-700 flex items-center gap-1.5 transition-all"
                >
                  <Filter className="w-3.5 h-3.5 text-indigo-600" />
                  <span>筛选条件：</span>
                  <span className="font-bold text-slate-900">
                    {reportSelectedMonths.length > 0 ? `${reportSelectedMonths.length} 个月份` : "全部月份"}
                    {" · "}
                    {reportSelectedProjects.length > 0 ? `${reportSelectedProjects.length} 个项目` : "全部项目"}
                  </span>
                </button>

                <div className="h-4 w-px bg-slate-200" />

                {/* 人天结算策略选择器 (4大商业核算策略) */}
                <div className="flex items-center gap-1.5 bg-slate-50 border border-slate-200/90 rounded-lg p-0.5">
                  <span className="text-[11px] font-bold text-slate-500 pl-2 pr-1 flex items-center gap-1">
                    <span>⚖️</span> 核算策略:
                  </span>
                  <select
                    value={currentStrategy}
                    onChange={(e) => {
                      const newStrat = e.target.value as "rebalance" | "min8_carry" | "cap8_carry" | "exact";
                      setCurrentStrategy(newStrat);
                      fetchSettlementReport(newStrat);
                      const stratLabels: Record<string, string> = {
                        rebalance: "🛡️ 已切换为：合规削峰填谷 (8h封顶·工作日平摊)",
                        min8_carry: "💎 已切换为：保底8小时 (不足8h按8h·超额顺延新天)",
                        cap8_carry: "⏩ 已切换为：封顶顺延 (单日封顶8h·超额顺延新天)",
                        exact: "⏱️ 已切换为：实际物理工时 (实报实销·同日合并)",
                      };
                      showToast(stratLabels[newStrat]);
                    }}
                    className="h-7 text-xs font-bold text-indigo-700 bg-white border border-indigo-200/70 rounded-md px-2 focus:outline-none focus:ring-1 focus:ring-indigo-500 cursor-pointer shadow-2xs"
                  >
                    <option value="rebalance">🛡️ 合规削峰填谷 (8h/天上限·平摊)</option>
                    <option value="min8_carry">💎 保底8小时模式 (不足8h按8h·超额顺延)</option>
                    <option value="cap8_carry">⏩ 封顶顺延模式 (单日8h封顶·顺延新天)</option>
                    <option value="exact">⏱️ 实际物理工时 (同日合并·实报实销)</option>
                  </select>
                </div>
              </div>

              {/* 右侧操作按钮 */}
              <div className="flex items-center gap-2.5">
                <button
                  onClick={() => handleBatchToggleInvoiced(true)}
                  className="h-8 px-3 rounded-lg bg-emerald-50 hover:bg-emerald-100 border border-emerald-200 text-emerald-700 text-xs font-bold flex items-center gap-1 transition-all active:scale-95"
                  title="将当前列表匹配的全部工时标记为已结算"
                >
                  <CheckCheck className="w-3.5 h-3.5 text-emerald-600" />
                  本期全标已结
                </button>
                <button
                  onClick={() => handleBatchToggleInvoiced(false)}
                  className="h-8 px-2.5 rounded-lg border border-slate-200 hover:bg-slate-50 text-slate-600 text-xs font-medium transition-all"
                  title="重置为待结算"
                >
                  重置待结
                </button>

                <div className="h-4 w-px bg-slate-200" />

                <button
                  onClick={() => {
                    setExportSelectedProjects(reportSelectedProjects.length > 0 ? reportSelectedProjects : projects.map(p => p.id));
                    setExportSelectedMonths(reportSelectedMonths);
                    setShowExportSettingsModal(true);
                  }}
                  className="h-8 px-3 rounded-lg bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-bold flex items-center gap-1.5 shadow-xs transition-all active:scale-95 cursor-pointer"
                  title="多选项目、多月份、独立文件与按月Sheet专业导出"
                >
                  <Download className="w-3.5 h-3.5" />
                  导出 Excel
                </button>

                <button
                  onClick={() => setShowReportExportModal(true)}
                  className="h-8 px-3 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg text-xs font-bold flex items-center gap-1.5 shadow-xs transition-all active:scale-95"
                >
                  <FileSpreadsheet className="w-3.5 h-3.5" />
                  对账单预览
                </button>
              </div>
            </div>

            {/* 3. 标准数据表格/工时流水列表 */}
            <div className="bg-white border border-slate-200/80 rounded-xl shadow-2xs overflow-hidden">
              <div className="px-4 py-3 border-b border-slate-100 flex items-center justify-between bg-slate-50/50">
                <div className="text-xs font-bold text-slate-700 flex items-center gap-2">
                  <span>对账明细流水</span>
                  <span className="text-[11px] font-normal text-slate-400">
                    （共 {settlementReport?.entries ? settlementReport.entries.length : 0} 笔记录）
                  </span>
                </div>
                <div className="text-[11px] text-slate-400">
                  提示：点击每行的「结算状态」按钮可快速切换状态
                </div>
              </div>

              {!settlementReport || settlementReport.entries.length === 0 ? (
                <div className="p-12 text-center space-y-2">
                  <div className="text-slate-300 text-3xl">📭</div>
                  <div className="text-xs text-slate-400">当前筛选条件下无工时明细流水</div>
                </div>
              ) : (
                <div className="overflow-x-auto max-h-[calc(100vh-290px)] overflow-y-auto">
                  <table className="w-full text-left border-collapse text-xs">
                    <thead className="bg-slate-50 sticky top-0 z-10 text-[11px] font-bold text-slate-500 uppercase tracking-wider border-b border-slate-200/80">
                      <tr>
                        <th className="py-2.5 px-4">日期与时间</th>
                        <th className="py-2.5 px-4">归属项目</th>
                        <th className="py-2.5 px-4">投入工时 (h)</th>
                        <th className="py-2.5 px-4">折算人天 (天)</th>
                        {!hideAmount && <th className="py-2.5 px-4">核算金额 (元)</th>}
                        <th className="py-2.5 px-4">工作内容 / 交付纪要</th>
                        <th className="py-2.5 px-4 text-center">结算状态</th>
                        <th className="py-2.5 px-4 text-right">操作</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-100 text-slate-700">
                      {settlementReport.entries.map((entry) => (
                        <tr key={entry.id} className="hover:bg-slate-50/70 transition-colors">
                          <td className="py-2.5 px-4 font-mono text-[11px] whitespace-nowrap text-slate-600">
                            <span className="font-bold text-slate-800">{entry.date_str}</span>
                            <span className="ml-1.5 text-slate-400">{entry.time_range}</span>
                          </td>
                          <td className="py-2.5 px-4 whitespace-nowrap font-medium text-slate-900">
                            {entry.project_name}
                          </td>
                          <td className="py-2.5 px-4 whitespace-nowrap text-slate-700 font-bold font-mono">
                            {formatToHalfHours(entry.duration_text)}
                          </td>
                          <td className="py-2.5 px-4 whitespace-nowrap font-semibold text-slate-800 font-mono">
                            {formatPureNumber(entry.days_text)}
                          </td>
                          {!hideAmount && (
                            <td className="py-2.5 px-4 whitespace-nowrap font-bold text-emerald-600 font-mono">
                              {formatPureNumber(entry.amount_text)}
                            </td>
                          )}
                          <td className="py-2.5 px-4 max-w-xs truncate text-slate-600" title={entry.summary_text}>
                            {entry.summary_text}
                          </td>
                          <td className="py-2.5 px-4 whitespace-nowrap text-center">
                            <button
                              onClick={() => handleToggleEntryInvoiced(entry.id, entry.is_invoiced)}
                              className={`px-2 py-0.5 rounded-full text-[10px] font-bold inline-flex items-center gap-1 border transition-all active:scale-95 ${
                                entry.is_invoiced
                                  ? "bg-emerald-50 text-emerald-700 border-emerald-300 hover:bg-emerald-100"
                                  : "bg-amber-50 text-amber-700 border-amber-300 hover:bg-amber-100"
                              }`}
                              title={entry.is_invoiced ? "点击重置为待结算" : "点击标记为已结算"}
                            >
                              {entry.is_invoiced ? (
                                <>
                                  <CheckCheck className="w-3 h-3 text-emerald-600" />
                                  已结算
                                </>
                              ) : (
                                <>
                                  <span className="w-1.5 h-1.5 rounded-full bg-amber-500 animate-pulse" />
                                  待结算
                                </>
                              )}
                            </button>
                          </td>
                          <td className="py-2.5 px-4 whitespace-nowrap text-right">
                            <button
                              onClick={() => {
                                setSelectedEntry(entry);
                                setShowDetailModal(true);
                              }}
                              className="text-xs text-indigo-600 hover:underline font-medium"
                            >
                              查看详情
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          </div>
        )}

        {/* 选项卡视图：系统与AI引擎配置 */}
        {activeTab === "settings" && (
          <div className="flex-1 p-6 overflow-y-auto">
            <div className="max-w-xl bg-white border border-slate-200/80 rounded-2xl p-6 shadow-sm space-y-5">
              <div>
                <h3 className="text-lg font-extrabold text-slate-900 flex items-center gap-2">
                  <Settings className="w-5 h-5 text-indigo-600" />
                  AI 引擎与代理服务器配置
                </h3>
                <p className="text-xs text-slate-500 mt-1">
                  支持开箱即用免 Key 的本地 agy / codex CLI，以及兼容 OpenAI / DeepSeek / Ollama 的自定义 HTTP 端点。
                </p>
              </div>

              {/* 核心 AI 调用模式切换 */}
              <div className="space-y-4 pt-1">
                <div>
                  <label className="text-xs font-bold text-slate-700 block mb-1.5">AI 调用模式 / Provider</label>
                  <div className="grid grid-cols-3 gap-2">
                    {[
                      { id: "agy", label: "agy CLI (推荐)", desc: "本地登录 免Key" },
                      { id: "openai", label: "OpenAI / HTTP", desc: "兼容端点 / 自定义API" },
                      { id: "codex", label: "codex CLI", desc: "本地 Codex 智能体" },
                    ].map((mode) => (
                      <button
                        key={mode.id}
                        type="button"
                        onClick={() => setSettingAiProvider(mode.id)}
                        className={`p-3 rounded-xl border text-left transition-all cursor-pointer ${
                          settingAiProvider === mode.id
                            ? "bg-indigo-50/70 border-indigo-500 ring-2 ring-indigo-500/20 text-indigo-900"
                            : "bg-white border-slate-200 hover:border-slate-300 text-slate-700"
                        }`}
                      >
                        <div className="text-xs font-bold">{mode.label}</div>
                        <div className="text-[10px] text-slate-400 mt-0.5">{mode.desc}</div>
                      </button>
                    ))}
                  </div>
                </div>

                {/* 代理服务器配置 (解决网络受限/境外AI端点直连失败) */}
                <div className="p-3.5 rounded-xl bg-amber-50/60 border border-amber-200/70 space-y-2">
                  <div className="flex items-center justify-between">
                    <label className="text-xs font-bold text-amber-900 flex items-center gap-1.5">
                      🌐 网络代理服务器 (Proxy)
                      <span className="text-[10px] font-normal px-1.5 py-0.5 rounded bg-amber-200/60 text-amber-800">可选</span>
                    </label>
                  </div>
                  <input
                    type="text"
                    value={settingAiProxy}
                    onChange={(e) => setSettingAiProxy(e.target.value)}
                    placeholder="http://127.0.0.1:7890 或 socks5://127.0.0.1:1080"
                    className="w-full h-9 px-3 rounded-lg border border-amber-300/80 text-xs text-slate-800 bg-white focus:outline-none focus:ring-2 focus:ring-amber-500/20 font-mono"
                  />
                  <p className="text-[11px] text-amber-800/80 leading-relaxed">
                    💡 当国内网络无法直接访问 OpenAI 或远程大模型服务时，填写此处的 HTTP/SOCKS5 代理。系统将自动对 HTTP 请求与 agy / codex 子进程注入代理环境。若留空则直连或继承系统环境变量。
                  </p>
                </div>

                {/* OpenAI / HTTP 兼容配置选项 */}
                {settingAiProvider === "openai" && (
                  <div className="space-y-3 p-4 rounded-xl bg-slate-50 border border-slate-200/80">
                    <div>
                      <label className="text-xs font-bold text-slate-700 block mb-1">API 端点 (Base URL)</label>
                      <input
                        type="text"
                        value={settingAiBase}
                        onChange={(e) => setSettingAiBase(e.target.value)}
                        placeholder="https://api.openai.com/v1 或 https://api.deepseek.com/v1"
                        className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 bg-white focus:outline-none focus:ring-1 focus:ring-indigo-500 font-mono"
                      />
                    </div>

                    <div>
                      <label className="text-xs font-bold text-slate-700 block mb-1">API Key</label>
                      <input
                        type="password"
                        value={settingAiKey}
                        onChange={(e) => setSettingAiKey(e.target.value)}
                        placeholder="sk-..."
                        className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 bg-white focus:outline-none focus:ring-1 focus:ring-indigo-500 font-mono"
                      />
                    </div>

                    <div>
                      <label className="text-xs font-bold text-slate-700 block mb-1">模型名称 (Model)</label>
                      <input
                        type="text"
                        value={settingAiModel}
                        onChange={(e) => setSettingAiModel(e.target.value)}
                        placeholder="gpt-4o-mini 或 deepseek-chat"
                        className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 bg-white focus:outline-none focus:ring-1 focus:ring-indigo-500 font-mono"
                      />
                    </div>
                  </div>
                )}

                {settingAiProvider === "agy" && (
                  <div className="p-3.5 rounded-xl bg-emerald-50 border border-emerald-200/80 text-emerald-800 text-xs leading-relaxed space-y-1">
                    <div className="font-bold flex items-center gap-1">
                      <span>✨ 本地 agy 驱动模式</span>
                    </div>
                    <p className="text-[11px] text-emerald-700">
                      MythManDay 将通过调用本地已授权的 <code className="px-1 py-0.5 rounded bg-emerald-100/70 font-mono text-[10px]">agy -p</code> 执行智能工时推算与音频交付总结。
                    </p>
                  </div>
                )}

                {/* 工时基准与规则配置 */}
                <div className="grid grid-cols-2 gap-3 pt-2 border-t border-slate-100">
                  <div>
                    <label className="text-xs font-bold text-slate-700 block mb-1">标准每日工时 (小时/人天)</label>
                    <input
                      type="number"
                      step="0.5"
                      value={settingHoursPerDay}
                      onChange={(e) => setSettingHoursPerDay(e.target.value)}
                      className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-1 focus:ring-indigo-500"
                    />
                  </div>
                  <div>
                    <label className="text-xs font-bold text-slate-700 block mb-1">无操作挂起判定 (AFK 分钟)</label>
                    <input
                      type="number"
                      step="1"
                      value={settingAfkMinutes}
                      onChange={(e) => setSettingAfkMinutes(e.target.value)}
                      className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-1 focus:ring-indigo-500"
                    />
                  </div>
                </div>

                <div className="pt-3 flex justify-end">
                  <button
                    type="button"
                    onClick={handleSaveSettings}
                    disabled={isSavingSettings}
                    className="px-5 py-2.5 rounded-xl bg-indigo-600 hover:bg-indigo-700 text-white text-xs font-bold shadow-sm transition-all cursor-pointer disabled:opacity-50 flex items-center gap-1.5"
                  >
                    {isSavingSettings ? "正在保存..." : "💾 保存全部设置"}
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}
      </main>

      {/* ---------------- 模态框 1: 发起会议沟通 ---------------- */}
      {showMeetingModal && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-200">
          <div className="w-full max-w-md bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4">
            <div className="flex items-center justify-between pb-2 border-b border-slate-100">
              <h3 className="text-base font-extrabold text-slate-900 flex items-center gap-2">
                <Mic className="w-4 h-4 text-blue-600" />
                发起即时会议与讨论
              </h3>
              <button
                onClick={() => setShowMeetingModal(false)}
                className="text-slate-400 hover:text-slate-700 text-sm font-bold"
              >
                ✕
              </button>
            </div>

            <div className="space-y-3">
              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">会议议题 / 讨论内容</label>
                <input
                  type="text"
                  value={meetingTopic}
                  onChange={(e) => setMeetingTopic(e.target.value)}
                  placeholder="例如：客户方案需求评审、架构重构对齐会"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500"
                />
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">关联归属项目</label>
                <select
                  value={meetingSelectedPid}
                  onChange={(e) => setMeetingSelectedPid(e.target.value)}
                  className="w-full h-9 px-2 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none"
                >
                  <option value="">⚡ 独立临时会议 (单独立项 · 不计入既有客户)</option>
                  {projects.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name} ({p.client})
                    </option>
                  ))}
                </select>
              </div>

              {/* 费率自动继承提示 */}
              <div className="p-2.5 rounded-lg bg-slate-50 border border-slate-200/80 text-[11px] text-slate-600 flex items-center justify-between">
                <span>💰 计费费率：</span>
                <span className="font-semibold text-slate-800">
                  {meetingSelectedPid
                    ? `${projects.find((p) => p.id === meetingSelectedPid)?.rate_info || "继承项目既定标准"}`
                    : "独立临时咨询 (标准: 350 CNY/小时)"}
                </span>
              </div>

              <div className="flex items-center gap-2 pt-2">
                <input
                  type="checkbox"
                  id="recordAudio"
                  checked={meetingRecordAudio}
                  onChange={(e) => setMeetingRecordAudio(e.target.checked)}
                  className="w-4 h-4 rounded text-blue-600"
                />
                <label htmlFor="recordAudio" className="text-xs font-medium text-slate-700">
                  🎙️ 同步开启麦克风全程录音（直存本地高保真 WAV）
                </label>
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-100">
              <button
                onClick={() => setShowMeetingModal(false)}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleStartMeeting}
                className="px-4 py-2 text-xs font-bold bg-blue-600 hover:bg-blue-700 text-white rounded-lg shadow-sm transition-all"
              >
                开始会议计时
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 2: 新建项目 ---------------- */}
      {showNewProjModal && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50">
          <div className="w-full max-w-md bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4">
            <div className="flex items-center justify-between pb-2 border-b border-slate-100">
              <h3 className="text-base font-extrabold text-slate-900">➕ 新建研发 / 计费项目</h3>
              <button
                onClick={() => setShowNewProjModal(false)}
                className="text-slate-400 hover:text-slate-700 text-sm font-bold"
              >
                ✕
              </button>
            </div>

            <div className="space-y-3">
              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">项目名称 (必填)</label>
                <input
                  type="text"
                  value={newProjName}
                  onChange={(e) => setNewProjName(e.target.value)}
                  placeholder="例如：客户A 跨境商城二次重构"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none"
                />
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">客户 / 委托方</label>
                <input
                  type="text"
                  value={newProjClient}
                  onChange={(e) => setNewProjClient(e.target.value)}
                  placeholder="例如：某某科技有限公司 / 张总"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:border-indigo-500"
                />
              </div>

              {/* 商业计费规则配置卡片 */}
              <div className="p-3 bg-slate-50 border border-slate-200/80 rounded-xl space-y-3">
                <div className="flex items-center justify-between pb-1 border-b border-slate-200/60">
                  <span className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
                    <span>💳</span> 商业计费规则
                  </span>
                  <span className="text-[10px] text-indigo-600 font-medium">支持灵活结算策略</span>
                </div>

                {/* 1. 计费方式切换 */}
                <div>
                  <label className="text-[11px] font-bold text-slate-600 block mb-1">计费方式</label>
                  <div className="grid grid-cols-2 gap-2">
                    <button
                      type="button"
                      onClick={() => {
                        setNewProjBillingType("daily");
                        if (newProjRate === "400") setNewProjRate("3000");
                      }}
                      className={`h-8 px-2.5 rounded-lg border text-xs font-bold flex items-center justify-center gap-1.5 transition-all ${
                        newProjBillingType === "daily"
                          ? "bg-indigo-600 border-indigo-600 text-white shadow-xs"
                          : "bg-white border-slate-200 text-slate-700 hover:bg-slate-100"
                      }`}
                    >
                      <span>📅</span> 按天 (默认最小0.5天)
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        setNewProjBillingType("hourly");
                        if (newProjRate === "3000") setNewProjRate("400");
                      }}
                      className={`h-8 px-2.5 rounded-lg border text-xs font-bold flex items-center justify-center gap-1.5 transition-all ${
                        newProjBillingType === "hourly"
                          ? "bg-indigo-600 border-indigo-600 text-white shadow-xs"
                          : "bg-white border-slate-200 text-slate-700 hover:bg-slate-100"
                      }`}
                    >
                      <span>⏱️</span> 按小时
                    </button>
                  </div>
                </div>

                {/* 2. 价格与币种 */}
                <div className="grid grid-cols-2 gap-2.5">
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">
                      {newProjBillingType === "daily" ? "人天单价 (价格/天)" : "时薪单价 (价格/小时)"}
                    </label>
                    <input
                      type="number"
                      value={newProjRate}
                      onChange={(e) => setNewProjRate(e.target.value)}
                      placeholder={newProjBillingType === "daily" ? "3000" : "400"}
                      className="w-full h-8 px-3 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-mono"
                    />
                  </div>
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">结算币种</label>
                    <select
                      value={newProjCurrency}
                      onChange={(e) => setNewProjCurrency(e.target.value)}
                      className="w-full h-8 px-2 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-medium"
                    >
                      <option value="CNY">CNY (人民币 ¥)</option>
                      <option value="USD">USD (美元 $)</option>
                      <option value="EUR">EUR (欧元 €)</option>
                      <option value="HKD">HKD (港币 HK$)</option>
                      <option value="JPY">JPY (日元 ¥)</option>
                      <option value="GBP">GBP (英镑 £)</option>
                    </select>
                  </div>
                </div>

                {/* 3. 舍入方式与最小刻度 */}
                <div className="grid grid-cols-2 gap-2.5">
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">舍入方式</label>
                    <select
                      value={newProjRoundingMode}
                      onChange={(e) => setNewProjRoundingMode(e.target.value as any)}
                      className="w-full h-8 px-2 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500"
                    >
                      <option value="ceil">进位 (不足进1/向上取整)</option>
                      <option value="half_up">四舍五入 (按刻度标准舍入)</option>
                      <option value="floor">舍去 (向下截断)</option>
                    </select>
                  </div>
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">
                      最小刻度 ({newProjBillingType === "daily" ? "天" : "小时"})
                    </label>
                    <select
                      value={newProjMinGranularity}
                      onChange={(e) => setNewProjMinGranularity(e.target.value)}
                      className="w-full h-8 px-2 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-mono"
                    >
                      <option value="0.5">0.5 {newProjBillingType === "daily" ? "天 (半天制)" : "小时 (半小时)"}</option>
                      <option value="1.0">1.0 {newProjBillingType === "daily" ? "天 (整天制)" : "小时 (整小时)"}</option>
                      <option value="0.25">0.25 {newProjBillingType === "daily" ? "天 (2小时)" : "小时 (15分钟)"}</option>
                      <option value="0.1">0.1 {newProjBillingType === "daily" ? "天 (精确制)" : "小时 (6分钟)"}</option>
                    </select>
                  </div>
                </div>
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">本地 Git 仓库路径 (可选，智能嗅探代码)</label>
                <input
                  type="text"
                  value={newProjGit}
                  onChange={(e) => setNewProjGit(e.target.value)}
                  placeholder="/home/beatle/dev/my-project"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:border-indigo-500"
                />
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-100">
              <button
                onClick={() => setShowNewProjModal(false)}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors cursor-pointer"
              >
                取消
              </button>
              <button
                onClick={handleCreateProject}
                className="px-4 py-2 text-xs font-bold bg-blue-600 hover:bg-blue-700 text-white rounded-lg shadow-sm transition-all cursor-pointer"
              >
                保存并创建项目
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 2.5: 配置与修改老项目商业计费规则 ---------------- */}
      {showEditProjModal && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-100">
          <div className="w-full max-w-lg bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4">
            <div className="flex items-center justify-between pb-3 border-b border-slate-100">
              <div className="flex items-center gap-2.5">
                <div className="w-9 h-9 rounded-xl bg-indigo-50 border border-indigo-100 flex items-center justify-center text-indigo-600 text-lg">
                  ⚙️
                </div>
                <div>
                  <h3 className="text-base font-extrabold text-slate-900 tracking-tight">配置项目商业计费规则</h3>
                  <p className="text-xs text-slate-500 font-medium">调整计费模式、费率、币种、舍入方式与阶梯刻度</p>
                </div>
              </div>
              <button
                onClick={() => setShowEditProjModal(false)}
                className="w-7 h-7 flex items-center justify-center text-slate-400 hover:text-slate-700 hover:bg-slate-100 rounded-lg text-sm font-bold transition-colors cursor-pointer"
              >
                ✕
              </button>
            </div>

            <div className="space-y-4">
              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">项目名称</label>
                <input
                  type="text"
                  value={editProjName}
                  onChange={(e) => setEditProjName(e.target.value)}
                  placeholder="例如：智能客服大模型中台架构研发"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-medium"
                />
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">客户 / 委托方名称</label>
                <input
                  type="text"
                  value={editProjClient}
                  onChange={(e) => setEditProjClient(e.target.value)}
                  placeholder="例如：北京某智能科技集团 / 某出海团队"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-medium"
                />
              </div>

              {/* 商业计费与结算规则 */}
              <div className="p-3.5 bg-slate-50/80 rounded-xl border border-slate-200/80 space-y-3">
                <div className="flex items-center justify-between border-b border-slate-200/60 pb-2">
                  <span className="text-xs font-extrabold text-slate-800 flex items-center gap-1.5">
                    <span>⚖️</span> 计费规则与精细阶梯控制
                  </span>
                  <span className="text-[10px] text-indigo-600 bg-indigo-50 px-1.5 py-0.5 rounded font-mono font-bold">
                    实时生效
                  </span>
                </div>

                {/* 1. 计费方式切换 */}
                <div>
                  <label className="text-[11px] font-bold text-slate-600 block mb-1">计费方式</label>
                  <div className="grid grid-cols-2 gap-2">
                    <button
                      type="button"
                      onClick={() => {
                        setEditProjBillingType("daily");
                        if (editProjRate === "400") setEditProjRate("3000");
                      }}
                      className={`h-8 px-2.5 rounded-lg border text-xs font-bold flex items-center justify-center gap-1.5 transition-all cursor-pointer ${
                        editProjBillingType === "daily"
                          ? "bg-indigo-600 border-indigo-600 text-white shadow-xs"
                          : "bg-white border-slate-200 text-slate-700 hover:bg-slate-100"
                      }`}
                    >
                      <span>📅</span> 按天 (默认最小0.5天)
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        setEditProjBillingType("hourly");
                        if (editProjRate === "3000") setEditProjRate("400");
                      }}
                      className={`h-8 px-2.5 rounded-lg border text-xs font-bold flex items-center justify-center gap-1.5 transition-all cursor-pointer ${
                        editProjBillingType === "hourly"
                          ? "bg-indigo-600 border-indigo-600 text-white shadow-xs"
                          : "bg-white border-slate-200 text-slate-700 hover:bg-slate-100"
                      }`}
                    >
                      <span>⏱️</span> 按小时
                    </button>
                  </div>
                </div>

                {/* 2. 价格与币种 */}
                <div className="grid grid-cols-2 gap-2.5">
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">
                      {editProjBillingType === "daily" ? "人天单价 (价格/天)" : "时薪单价 (价格/小时)"}
                    </label>
                    <input
                      type="number"
                      value={editProjRate}
                      onChange={(e) => setEditProjRate(e.target.value)}
                      placeholder={editProjBillingType === "daily" ? "3000" : "400"}
                      className="w-full h-8 px-3 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-mono"
                    />
                  </div>
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">结算币种</label>
                    <select
                      value={editProjCurrency}
                      onChange={(e) => setEditProjCurrency(e.target.value)}
                      className="w-full h-8 px-2 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-medium"
                    >
                      <option value="CNY">CNY (人民币 ¥)</option>
                      <option value="USD">USD (美元 $)</option>
                      <option value="EUR">EUR (欧元 €)</option>
                      <option value="HKD">HKD (港币 HK$)</option>
                      <option value="JPY">JPY (日元 ¥)</option>
                      <option value="GBP">GBP (英镑 £)</option>
                    </select>
                  </div>
                </div>

                {/* 3. 舍入方式与最小刻度 */}
                <div className="grid grid-cols-2 gap-2.5">
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">舍入方式</label>
                    <select
                      value={editProjRoundingMode}
                      onChange={(e) => setEditProjRoundingMode(e.target.value as any)}
                      className="w-full h-8 px-2 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500"
                    >
                      <option value="ceil">进位 (不足进1/向上取整)</option>
                      <option value="half_up">四舍五入 (按刻度标准舍入)</option>
                      <option value="floor">舍去 (向下截断)</option>
                    </select>
                  </div>
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">
                      最小刻度 ({editProjBillingType === "daily" ? "天" : "小时"})
                    </label>
                    <select
                      value={editProjMinGranularity}
                      onChange={(e) => setEditProjMinGranularity(e.target.value)}
                      className="w-full h-8 px-2 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-mono"
                    >
                      <option value="0.5">0.5 {editProjBillingType === "daily" ? "天 (半天制)" : "小时 (半小时)"}</option>
                      <option value="1.0">1.0 {editProjBillingType === "daily" ? "天 (整天制)" : "小时 (整小时)"}</option>
                      <option value="0.25">0.25 {editProjBillingType === "daily" ? "天 (2小时)" : "小时 (15分钟)"}</option>
                      <option value="0.1">0.1 {editProjBillingType === "daily" ? "天 (精确制)" : "小时 (6分钟)"}</option>
                    </select>
                  </div>
                </div>
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">本地 Git 仓库路径 (可选，智能嗅探代码)</label>
                <input
                  type="text"
                  value={editProjGit}
                  onChange={(e) => setEditProjGit(e.target.value)}
                  placeholder="/home/beatle/dev/my-project"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:border-indigo-500"
                />
              </div>
            </div>

            <div className="flex items-center justify-between gap-2 pt-3 border-t border-slate-100">
              <button
                type="button"
                onClick={() => {
                  const target = projects.find((p) => p.id === editProjId);
                  if (target) {
                    setShowEditProjModal(false);
                    setProjectToDelete(target);
                  }
                }}
                className="px-3 py-2 text-xs font-bold text-rose-600 hover:bg-rose-50 border border-rose-200 rounded-lg transition-colors cursor-pointer flex items-center gap-1.5"
              >
                <Trash2 className="w-3.5 h-3.5" />
                删除此项目
              </button>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setShowEditProjModal(false)}
                  className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors cursor-pointer"
                >
                  取消
                </button>
                <button
                  onClick={handleUpdateProject}
                  className="px-4 py-2 text-xs font-bold bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg shadow-sm transition-all cursor-pointer"
                >
                  保存规则配置
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框: 删除项目二次确认 ---------------- */}
      {projectToDelete && (
        <div className="fixed inset-0 bg-slate-900/50 backdrop-blur-xs flex items-center justify-center p-4 z-50">
          <div className="w-full max-w-md bg-white rounded-2xl border border-slate-200/90 shadow-2xl p-6 space-y-4 animate-in fade-in zoom-in duration-150">
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl bg-rose-50 border border-rose-100 flex items-center justify-center text-rose-600 shrink-0">
                <Trash2 className="w-5 h-5 text-rose-600" />
              </div>
              <div className="min-w-0">
                <h3 className="text-base font-extrabold text-slate-900 tracking-tight">删除项目确认</h3>
                <p className="text-xs text-slate-500 truncate">
                  {projectToDelete.name} ({projectToDelete.client})
                </p>
              </div>
            </div>

            <div className="bg-rose-50/60 border border-rose-200/60 rounded-xl p-3.5 text-xs text-rose-800 space-y-1.5 leading-relaxed">
              <p className="font-semibold flex items-center gap-1">
                <span>⚠️</span> 危险操作警告：
              </p>
              <p>
                即将永久删除项目「<span className="font-bold text-slate-900">{projectToDelete.name}</span>」！
              </p>
              <p className="text-[11px] text-rose-700">
                此操作将同时清空该项目所有的单日工时记录、结算凭证、便签纪要与关联数据，且<strong>无法撤销恢复</strong>。
              </p>
            </div>

            <div className="flex items-center justify-end gap-2.5 pt-2">
              <button
                type="button"
                onClick={() => setProjectToDelete(null)}
                disabled={isDeletingProject}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors cursor-pointer"
              >
                取消
              </button>
              <button
                type="button"
                onClick={() => handleDeleteProject(projectToDelete)}
                disabled={isDeletingProject}
                className="px-4 py-2 text-xs font-bold bg-rose-600 hover:bg-rose-700 text-white rounded-lg shadow-sm transition-all cursor-pointer flex items-center gap-1.5 disabled:opacity-50"
              >
                <Trash2 className="w-3.5 h-3.5" />
                {isDeletingProject ? "正在删除..." : "确认永久删除"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 3: 结构化单日工时与工作交付详情 (选项 A) ---------------- */}
      {showDetailModal && selectedEntry && (
        <div className="fixed inset-0 bg-slate-900/50 backdrop-blur-xs flex items-center justify-center p-4 z-50">
          <div className="w-full max-w-xl bg-white rounded-2xl border border-slate-200/90 shadow-2xl p-6 space-y-5 animate-in fade-in zoom-in duration-150">
            {/* 顶部标题栏与关闭 */}
            <div className="flex items-center justify-between pb-3 border-b border-slate-100">
              <div className="flex items-center gap-2.5">
                <div className="w-9 h-9 rounded-xl bg-indigo-50 border border-indigo-100 flex items-center justify-center text-indigo-600 text-lg">
                  📅
                </div>
                <div>
                  <h3 className="text-base font-extrabold text-slate-900 tracking-tight">单日工时与工作交付详情</h3>
                  <p className="text-xs text-slate-500 font-medium">
                    {selectedEntry.project_name} · {selectedEntry.date_str} {selectedEntry.time_range !== "全天标准" ? `(${selectedEntry.time_range})` : ""}
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <span
                  className={`px-2.5 py-1 rounded-full text-[11px] font-bold border ${
                    selectedEntry.is_invoiced
                      ? "bg-emerald-50 text-emerald-700 border-emerald-200/80"
                      : "bg-amber-50 text-amber-700 border-amber-200/80"
                  }`}
                >
                  {selectedEntry.is_invoiced ? "✓ 已结清" : "⏳ 待结清"}
                </span>
                <button
                  onClick={() => setShowDetailModal(false)}
                  className="w-7 h-7 flex items-center justify-center text-slate-400 hover:text-slate-700 hover:bg-slate-100 rounded-lg text-sm font-bold transition-colors"
                >
                  ✕
                </button>
              </div>
            </div>

            {/* 核算指标网格卡片 */}
            <div className={`grid gap-3 p-3.5 bg-slate-50/80 border border-slate-200/80 rounded-xl ${hideAmount ? "grid-cols-2" : "grid-cols-3"}`}>
              <div>
                <span className="text-[11px] font-semibold text-slate-400 block mb-0.5">核算工时</span>
                <div className="text-sm font-extrabold text-slate-900 font-mono flex items-baseline gap-1">
                  {selectedEntry.duration_text}
                </div>
              </div>
              <div>
                <span className="text-[11px] font-semibold text-slate-400 block mb-0.5">折算人天</span>
                <div className="text-sm font-extrabold text-indigo-600 font-mono">
                  {selectedEntry.days_text}
                </div>
              </div>
              {!hideAmount && (
                <div>
                  <span className="text-[11px] font-semibold text-slate-400 block mb-0.5">应收结算金额</span>
                  <div className="text-sm font-extrabold text-emerald-600 font-mono">
                    {selectedEntry.amount_text}
                  </div>
                </div>
              )}
            </div>

            {/* 录音/凭证组件 */}
            {selectedEntry.has_audio && (
              <div className="p-3 bg-indigo-50/60 border border-indigo-100/80 rounded-xl flex items-center justify-between">
                <span className="text-xs font-semibold text-indigo-900 flex items-center gap-2">
                  <Volume2 className="w-4 h-4 text-indigo-600" />
                  已留存当日会议录音/工时语音凭证
                </span>
                <button
                  onClick={() => handleOpenAudioPlayer(selectedEntry.audio_path, `${selectedEntry.date_str} 会议录音凭证`)}
                  className="px-3.5 py-1.5 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg text-xs font-bold shadow-xs transition-colors flex items-center gap-1.5 cursor-pointer"
                >
                  <Volume2 className="w-3.5 h-3.5" />
                  播放录音
                </button>
              </div>
            )}

            {/* 结构化工作内容与交付清单 */}
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-xs font-bold text-slate-700 flex items-center gap-1.5">
                  <span>💼</span> 交付工作内容清单
                </span>
                <span className="text-[11px] text-slate-400 font-medium">按单项去重归集</span>
              </div>

              <div className="p-3.5 rounded-xl bg-slate-50 border border-slate-200/80 max-h-60 overflow-y-auto space-y-2">
                {(() => {
                  // 智能清洗与分项切分：支持分号、句号、换行，并剥除系统算法标签
                  const cleanText = (selectedEntry.summary_text || "")
                    .replace(/\[削峰回填\]\s*/g, "")
                    .replace(/\[超额顺延\]\s*/g, "");
                  
                  const items = cleanText
                    .split(/[；;。\n]+/)
                    .map((s) => s.trim())
                    .filter((s) => s.length > 0);

                  if (items.length === 0) {
                    return <div className="text-xs text-slate-400 italic py-2 text-center">暂无详细工作纪要描述</div>;
                  }

                  return items.map((item, idx) => (
                    <div
                      key={idx}
                      className="p-2.5 bg-white border border-slate-200/70 rounded-lg text-xs text-slate-800 leading-relaxed flex items-start gap-2 shadow-2xs hover:border-indigo-200 transition-colors"
                    >
                      <span className="w-4 h-4 rounded-full bg-slate-100 text-slate-500 font-bold text-[10px] flex items-center justify-center shrink-0 mt-0.5">
                        {idx + 1}
                      </span>
                      <span className="flex-1 font-medium">{item}</span>
                    </div>
                  ));
                })()}
              </div>
            </div>

            {/* 底部按钮栏 */}
            <div className="flex items-center justify-between pt-2 border-t border-slate-100">
              <div className="flex items-center gap-2">
                <button
                  onClick={() => {
                    const { summary: cleanSummary } = splitSummaryAndTranscript(selectedEntry.summary_text || "");
                    const cleanText = cleanSummary
                      .replace(/\[削峰回填\]\s*/g, "")
                      .replace(/\[超额顺延\]\s*/g, "");
                    navigator.clipboard.writeText(cleanText);
                    showToast("交付清单已整理并复制到剪贴板！");
                  }}
                  className="px-3.5 py-2 bg-indigo-50 hover:bg-indigo-100 text-indigo-700 text-xs font-bold rounded-lg border border-indigo-200/60 flex items-center gap-1.5 transition-colors cursor-pointer"
                >
                  <Copy className="w-3.5 h-3.5" />
                  复制交付清单
                </button>

                {(() => {
                  const { transcript: rawTranscript } = splitSummaryAndTranscript(selectedEntry.summary_text || "");
                  if (!rawTranscript) return null;
                  return (
                    <button
                      onClick={() => setActiveTranscriptModal({
                        title: `📅 ${selectedEntry.date_str} 会议全量转写实录`,
                        transcript: rawTranscript,
                      })}
                      className="px-3 py-2 bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-semibold rounded-lg border border-slate-200 flex items-center gap-1.5 transition-colors cursor-pointer"
                      title="打开独立窗口查看数万字逐字发言文稿"
                    >
                      <FileText className="w-3.5 h-3.5 text-slate-500" />
                      查看全量实录 ({rawTranscript.length}字)
                    </button>
                  );
                })()}
              </div>
              
              <button
                onClick={() => setShowDetailModal(false)}
                className="px-5 py-2 bg-slate-100 hover:bg-slate-200 text-slate-700 font-bold text-xs rounded-lg transition-colors cursor-pointer"
              >
                关闭
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 4: 手动补录工时 ---------------- */}
      {showManualEntryModal && activeProject && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50">
          <div className="w-full max-w-md bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4">
            <div className="flex items-center justify-between pb-2 border-b border-slate-100">
              <h3 className="text-base font-extrabold text-slate-900 flex items-center gap-2">
                <PlusCircle className="w-4 h-4 text-indigo-600" />
                补录工时投入
              </h3>
              <button
                onClick={() => setShowManualEntryModal(false)}
                className="text-slate-400 hover:text-slate-700 text-sm font-bold"
              >
                ✕
              </button>
            </div>

            <div className="space-y-3">
              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">归属项目</label>
                <div className="p-2.5 rounded-lg bg-slate-50 border border-slate-200 text-xs font-bold text-slate-800">
                  {activeProject.name} <span className="font-normal text-slate-500">({activeProject.client})</span>
                </div>
              </div>

              {/* Git 仓库信息展示与快捷配置 */}
              {activeProject.git_repo_path && !showEditRepoInput ? (
                <div className="flex items-center justify-between text-[11px] text-slate-600 bg-slate-50 border border-slate-200/90 rounded-lg px-2.5 py-1.5">
                  <div className="flex items-center gap-1.5 min-w-0">
                    <span className="font-bold text-slate-700 shrink-0">Git 仓库:</span>
                    <span className="truncate font-mono text-[10px] text-slate-500" title={activeProject.git_repo_path}>
                      {activeProject.git_repo_path}
                    </span>
                  </div>
                  <button
                    type="button"
                    onClick={() => setShowEditRepoInput(true)}
                    className="text-indigo-600 hover:text-indigo-800 text-[11px] font-semibold ml-2 shrink-0 cursor-pointer"
                  >
                    更换
                  </button>
                </div>
              ) : (
                <div className="space-y-1 bg-indigo-50/40 border border-indigo-100 rounded-xl p-2.5">
                  <div className="flex items-center justify-between text-[11px]">
                    <span className="font-bold text-slate-700 flex items-center gap-1">
                      <span>📁</span> 本地 Git 仓库路径
                    </span>
                    {activeProject.git_repo_path && (
                      <button
                        type="button"
                        onClick={() => setShowEditRepoInput(false)}
                        className="text-slate-400 hover:text-slate-600 text-[10px] cursor-pointer"
                      >
                        收起
                      </button>
                    )}
                  </div>
                  <input
                    type="text"
                    value={manualEntryRepoPath}
                    onChange={(e) => setManualEntryRepoPath(e.target.value)}
                    placeholder="/home/beatle/dev/research"
                    className="w-full h-8 px-2.5 rounded-lg border border-slate-200 text-xs text-slate-800 bg-white focus:outline-none focus:ring-1 focus:ring-indigo-500 font-mono"
                  />
                  <p className="text-[10px] text-slate-400">
                    填入项目本地代码路径后，系统将自动嗅探对应日期的 Git Commit 提交记录
                  </p>
                </div>
              )}

              <ModernDatePicker
                label="工作日期"
                value={manualEntryDate}
                onChange={setManualEntryDate}
              />

              <div className="grid grid-cols-2 gap-3">
                <ModernTimePicker
                  label="开始时间"
                  value={manualEntryStartTime}
                  onChange={setManualEntryStartTime}
                />
                <ModernTimePicker
                  label="结束时间"
                  value={manualEntryEndTime}
                  onChange={setManualEntryEndTime}
                />
              </div>

              {/* AI 推荐时段快速切换 */}
              {smartTimeSlots.length > 0 && (
                <div className="flex flex-wrap items-center gap-1.5 -mt-1">
                  <span className="text-[11px] text-slate-400 font-medium">推荐时段:</span>
                  {smartTimeSlots.map((slot, idx) => {
                    const isSelected = manualEntryStartTime === slot.start && manualEntryEndTime === slot.end;
                    return (
                      <button
                        key={idx}
                        type="button"
                        onClick={() => {
                          setManualEntryStartTime(slot.start);
                          setManualEntryEndTime(slot.end);
                        }}
                        className={`px-2 py-0.5 rounded text-[11px] font-medium transition-all cursor-pointer border ${
                          isSelected
                            ? "bg-indigo-600 text-white border-indigo-600 shadow-2xs font-bold"
                            : "bg-indigo-50/70 hover:bg-indigo-100 text-indigo-700 border-indigo-200/80"
                        }`}
                      >
                        {slot.label} ({slot.start}~{slot.end})
                      </button>
                    );
                  })}
                </div>
              )}

              {/* AI 评估依据提示 */}
              {smartAiReason && (
                <div className="p-2 rounded-lg bg-indigo-50/60 border border-indigo-100/80 flex items-start gap-1.5 text-[11px] text-indigo-900 leading-snug">
                  <span className="shrink-0 text-indigo-500">💡</span>
                  <div className="flex-1">
                    <span className="font-semibold text-indigo-700">AI 推算依据：</span>
                    {smartAiReason}
                  </div>
                </div>
              )}

              <div>
                <div className="flex items-center justify-between mb-1">
                  <div className="flex items-center gap-2">
                    <label className="text-xs font-bold text-slate-700">工作内容 / 交付说明</label>
                    {manualEntrySummary && (
                      <button
                        type="button"
                        onClick={() => {
                          const parts = manualEntrySummary
                            .split(/[；;。\n]+/)
                            .map((s) => s.trim())
                            .filter((s) => s.length > 0);
                          if (parts.length > 1) {
                            const formatted = parts
                              .map((p, i) => {
                                const clean = p.replace(/^(\d+[\.、\s\-•]+|\-|\•)/, "").trim();
                                return `${i + 1}. ${clean}`;
                              })
                              .join("\n");
                            setManualEntrySummary(formatted);
                          }
                        }}
                        className="text-[10px] text-indigo-600 hover:text-indigo-800 hover:underline cursor-pointer font-medium"
                        title="将当前长句按标点智能分行格式化为条目列表"
                      >
                        ⚡ 转换为条目式
                      </button>
                    )}
                  </div>
                  <button
                    type="button"
                    onClick={handleSmartFillGit}
                    disabled={isSmartFilling}
                    className="px-2.5 py-1 rounded-md bg-indigo-50 hover:bg-indigo-100 border border-indigo-200 text-indigo-700 text-[11px] font-bold flex items-center gap-1.5 transition-all cursor-pointer disabled:opacity-50 shadow-2xs"
                    title="自动根据选择的工作日期和时间段，从 Git 版本库拉取提交记录并由 AI 结构化提炼为条目清单"
                  >
                    <Sparkles className={`w-3.5 h-3.5 text-indigo-600 ${isSmartFilling ? "animate-spin" : ""}`} />
                    {isSmartFilling ? "正在拉取版本库..." : "✨ 智能从版本库填充"}
                  </button>
                </div>
                <textarea
                  value={manualEntrySummary}
                  onChange={(e) => setManualEntrySummary(e.target.value)}
                  placeholder="1. 完成核心业务模块开发与重构&#10;2. 支持数据导入导出及阶段约束&#10;3. 修复潜在核算偏差问题 [0d0e0667]"
                  rows={5}
                  className="w-full p-2.5 rounded-xl border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-1 focus:ring-indigo-500 leading-relaxed font-sans font-mono"
                />
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-100">
              <button
                onClick={() => setShowManualEntryModal(false)}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleLogManualEntry}
                className="px-4 py-2 text-xs font-bold bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg shadow-sm transition-all"
              >
                确认补录工时
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 5: 补录会议与上传录音 ---------------- */}
      {showManualMeetingModal && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50">
          <div className="w-full max-w-md bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4">
            <div className="flex items-center justify-between pb-2 border-b border-slate-100">
              <h3 className="text-base font-extrabold text-slate-900 flex items-center gap-2">
                <Upload className="w-4 h-4 text-indigo-600" />
                补录会议与录音凭证
              </h3>
              <button
                onClick={() => setShowManualMeetingModal(false)}
                className="text-slate-400 hover:text-slate-700 text-sm font-bold"
              >
                ✕
              </button>
            </div>

            <div className="space-y-3">
              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">会议议题</label>
                <input
                  type="text"
                  value={manualMeetingTopic}
                  onChange={(e) => setManualMeetingTopic(e.target.value)}
                  placeholder="例如：跨部门对接评审会、业务架构宣讲"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-1 focus:ring-indigo-500"
                />
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">归属项目</label>
                <select
                  value={manualMeetingPid}
                  onChange={(e) => setManualMeetingPid(e.target.value)}
                  className="w-full h-9 px-2 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none"
                >
                  <option value="">⚡ 独立临时会议 (单独立项)</option>
                  {projects.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name} ({p.client})
                    </option>
                  ))}
                </select>
              </div>

              <ModernDatePicker
                label="会议日期"
                value={manualMeetingDate}
                onChange={setManualMeetingDate}
              />

              <div className="grid grid-cols-2 gap-3">
                <ModernTimePicker
                  label="开始时间"
                  value={manualMeetingStartTime}
                  onChange={setManualMeetingStartTime}
                />
                <ModernTimePicker
                  label="结束时间"
                  value={manualMeetingEndTime}
                  onChange={setManualMeetingEndTime}
                />
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">会议费率 (元/小时)</label>
                <input
                  type="number"
                  value={manualMeetingRate}
                  onChange={(e) => setManualMeetingRate(e.target.value)}
                  placeholder="350"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none"
                />
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">本地已有录音文件路径 (支持 .wav / .mp3 / .m4a)</label>
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    value={manualMeetingAudioPath}
                    onChange={(e) => setManualMeetingAudioPath(e.target.value)}
                    placeholder="/home/beatle/meeting.wav 或留空"
                    className="flex-1 h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 font-mono focus:outline-none"
                  />
                  <input
                    type="file"
                    id="audioFileInput"
                    accept="audio/*,.wav,.mp3,.m4a"
                    className="hidden"
                    onChange={(e) => {
                      const file = e.target.files?.[0];
                      if (file) {
                        // 在现代浏览器中无法直接读取完整物理路径，支持由用户确认或输入
                        const p = (file as any).path || file.name;
                        setManualMeetingAudioPath(p);
                      }
                    }}
                  />
                  <label
                    htmlFor="audioFileInput"
                    className="h-9 px-3 bg-slate-100 hover:bg-slate-200 border border-slate-200 rounded-lg text-xs font-bold text-slate-700 flex items-center cursor-pointer transition-colors"
                  >
                    浏览
                  </label>
                </div>
                <p className="text-[10px] text-slate-400 mt-1">
                  💡 录音文件将被安全归档至 MythManDay 数据目录，并在补登完成后支持一键播放与 AI 纪要转写。
                </p>
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-100">
              <button
                onClick={() => setShowManualMeetingModal(false)}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleLogManualMeeting}
                className="px-4 py-2 text-xs font-bold bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg shadow-sm transition-all"
              >
                保存并归档会议
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 6: 快捷工作便签弹窗 ---------------- */}
      {showNoteModal && activeProject && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50">
          <div className="w-full max-w-md bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4">
            <div className="flex items-center justify-between pb-2 border-b border-slate-100">
              <h3 className="text-base font-extrabold text-slate-900 flex items-center gap-2">
                <StickyNote className="w-4 h-4 text-amber-500" />
                记便签 · {activeProject.name}
              </h3>
              <button
                onClick={() => setShowNoteModal(false)}
                className="text-slate-400 hover:text-slate-700 text-sm font-bold"
              >
                ✕
              </button>
            </div>

            <div className="space-y-3">
              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">
                  输入工作随笔、需求变更或沟通速记
                </label>
                <textarea
                  value={noteInput}
                  onChange={(e) => setNoteInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                      handleAddNote();
                      setShowNoteModal(false);
                    }
                  }}
                  placeholder="例如：客户提到结算模块下周三前必须支持按月开票导出；稍后跟进修复验证。"
                  rows={4}
                  className="w-full p-3 rounded-xl bg-slate-50 border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500 leading-relaxed"
                  autoFocus
                />
                <p className="text-[10px] text-slate-400 mt-1">快捷键: 按 Ctrl+Enter 可直接提交保存</p>
              </div>

              {/* 历史便签快速预览 */}
              {notes.length > 0 && (
                <div className="pt-2 border-t border-slate-100 space-y-2">
                  <div className="text-[11px] font-bold text-slate-500">已存备忘（{notes.length} 条）</div>
                  <div className="max-h-36 overflow-y-auto space-y-1.5 pr-1">
                    {notes.slice(0, 5).map((n) => (
                      <div
                        key={n.id}
                        className="p-2 rounded-lg bg-slate-50 border border-slate-200/60 flex items-center justify-between text-[11px]"
                      >
                        <span className="text-slate-700 truncate mr-2">
                          <span className="text-indigo-600 font-mono mr-1">[{n.time_text}]</span>
                          {n.content}
                        </span>
                        <button
                          onClick={() => handleDeleteNote(n.id)}
                          className="text-slate-400 hover:text-rose-500 shrink-0 p-0.5"
                          title="删除"
                        >
                          <Trash2 className="w-3 h-3" />
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-100">
              <button
                onClick={() => setShowNoteModal(false)}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
              >
                关闭
              </button>
              <button
                onClick={async () => {
                  if (noteInput.trim()) {
                    await handleAddNote();
                    setShowNoteModal(false);
                  }
                }}
                className="px-4 py-2 text-xs font-bold bg-amber-500 hover:bg-amber-600 text-white rounded-lg shadow-sm transition-all"
              >
                保存便签
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 6: 高级多维筛选弹窗 ---------------- */}
      {showFilterModal && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-100">
          <div className="w-full max-w-lg bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-5">
            <div className="flex items-center justify-between pb-3 border-b border-slate-100">
              <h3 className="text-sm font-extrabold text-slate-900 flex items-center gap-2">
                <Filter className="w-4 h-4 text-indigo-600" />
                多维对账条件筛选
              </h3>
              <button
                onClick={() => setShowFilterModal(false)}
                className="text-slate-400 hover:text-slate-700 text-sm font-bold"
              >
                ✕
              </button>
            </div>

            <div className="space-y-4">
              {/* 跨月份多选 */}
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-bold text-slate-700">核算月份（支持跨多月）</label>
                  {reportSelectedMonths.length > 0 && (
                    <button
                      onClick={() => setReportSelectedMonths([])}
                      className="text-[11px] text-indigo-600 hover:underline"
                    >
                      清空月份 (全选历史)
                    </button>
                  )}
                </div>
                <div className="flex flex-wrap gap-1.5 max-h-28 overflow-y-auto p-2 bg-slate-50 border border-slate-200 rounded-xl">
                  {(() => {
                    const monthsList: string[] = [];
                    const d = new Date();
                    for (let i = 0; i < 12; i++) {
                      const ym = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
                      monthsList.push(ym);
                      d.setMonth(d.getMonth() - 1);
                    }
                    return monthsList.map((m) => {
                      const isChecked = reportSelectedMonths.includes(m);
                      return (
                        <button
                          key={m}
                          onClick={() => {
                            const updated = isChecked
                              ? reportSelectedMonths.filter((x) => x !== m)
                              : [...reportSelectedMonths, m];
                            setReportSelectedMonths(updated);
                          }}
                          className={`px-2.5 py-1 rounded-lg text-xs font-mono transition-all ${
                            isChecked
                              ? "bg-indigo-600 text-white font-bold shadow-2xs"
                              : "bg-white text-slate-700 border border-slate-200 hover:bg-slate-100"
                          }`}
                        >
                          {m}
                        </button>
                      );
                    });
                  })()}
                </div>
                <p className="text-[10px] text-slate-400">未选择任何月份时，自动计算全部历史发生工时</p>
              </div>

              {/* 跨项目多选 */}
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-bold text-slate-700">归属项目（支持跨项目）</label>
                  {reportSelectedProjects.length > 0 && (
                    <button
                      onClick={() => setReportSelectedProjects([])}
                      className="text-[11px] text-indigo-600 hover:underline"
                    >
                      包含全部项目
                    </button>
                  )}
                </div>
                <div className="flex flex-wrap gap-1.5 max-h-32 overflow-y-auto p-2 bg-slate-50 border border-slate-200 rounded-xl">
                  {projects.map((p) => {
                    const isChecked = reportSelectedProjects.includes(p.id);
                    return (
                      <button
                        key={p.id}
                        onClick={() => {
                          const updated = isChecked
                            ? reportSelectedProjects.filter((x) => x !== p.id)
                            : [...reportSelectedProjects, p.id];
                          setReportSelectedProjects(updated);
                        }}
                        className={`px-2.5 py-1 rounded-lg text-xs transition-all truncate max-w-[160px] ${
                          isChecked
                            ? "bg-indigo-600 text-white font-bold shadow-2xs"
                            : "bg-white text-slate-700 border border-slate-200 hover:bg-slate-100"
                        }`}
                        title={p.name}
                      >
                        {p.name}
                      </button>
                    );
                  })}
                </div>
                <p className="text-[10px] text-slate-400">未选择项目时，自动聚合全部已立项项目流水</p>
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-100">
              <button
                onClick={() => setShowFilterModal(false)}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
              >
                取消
              </button>
              <button
                onClick={() => {
                  fetchSettlementReport();
                  setShowFilterModal(false);
                  showToast("筛选条件已更新！");
                }}
                className="px-4 py-2 text-xs font-bold bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg shadow-sm transition-all"
              >
                应用筛选
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 7: 对账报表预览与导出弹窗 ---------------- */}
      {showReportExportModal && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-100">
          <div className="w-full max-w-3xl bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4 max-h-[85vh] flex flex-col">
            <div className="flex items-center justify-between pb-3 border-b border-slate-100 shrink-0">
              <div className="flex items-center gap-2">
                <FileSpreadsheet className="w-4 h-4 text-indigo-600" />
                <h3 className="text-sm font-extrabold text-slate-900">
                  多项目人天结算对账单预览
                </h3>
              </div>
              <div className="flex items-center gap-2">
                {/* 视图切换 */}
                <div className="flex items-center bg-slate-100 rounded-lg p-0.5 border border-slate-200/60 text-xs">
                  <button
                    onClick={() => setReportViewMode("preview")}
                    className={`px-2.5 py-1 rounded-md transition-all ${
                      reportViewMode === "preview" ? "bg-white text-slate-900 font-bold shadow-2xs" : "text-slate-500 hover:text-slate-800"
                    }`}
                  >
                    排版预览
                  </button>
                  <button
                    onClick={() => setReportViewMode("raw")}
                    className={`px-2.5 py-1 rounded-md transition-all ${
                      reportViewMode === "raw" ? "bg-white text-slate-900 font-bold shadow-2xs" : "text-slate-500 hover:text-slate-800"
                    }`}
                  >
                    Markdown 源码
                  </button>
                </div>

                <button
                  onClick={() => setShowReportExportModal(false)}
                  className="text-slate-400 hover:text-slate-700 text-sm font-bold ml-2"
                >
                  ✕
                </button>
              </div>
            </div>

            {/* 报表内容 */}
            <div className="flex-1 overflow-y-auto">
              {reportViewMode === "preview" ? (
                <div
                  className="p-5 rounded-xl bg-slate-50 border border-slate-200 text-xs markdown-body"
                  dangerouslySetInnerHTML={{
                    __html: renderMarkdown(settlementReport?.markdown || "正在聚合对账单..."),
                  }}
                />
              ) : (
                <div className="p-4 rounded-xl bg-slate-50 border border-slate-200 font-mono text-xs text-slate-800 whitespace-pre-wrap leading-relaxed">
                  {settlementReport?.markdown || "正在聚合对账单..."}
                </div>
              )}
            </div>

            {/* 底部操作区 */}
            <div className="flex items-center justify-between pt-3 border-t border-slate-100 shrink-0">
              <span className="text-[11px] text-slate-400">
                可直接复制文本发给客户、财务或作为项目周报/月报交付凭据
              </span>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setShowReportExportModal(false)}
                  className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
                >
                  关闭
                </button>
                <button
                  onClick={() => {
                    setShowReportExportModal(false);
                    setExportSelectedProjects(reportSelectedProjects.length > 0 ? reportSelectedProjects : projects.map(p => p.id));
                    setExportSelectedMonths(reportSelectedMonths);
                    setShowExportSettingsModal(true);
                  }}
                  className="px-4 py-2 text-xs font-bold bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg shadow-sm flex items-center gap-1.5 transition-all cursor-pointer"
                >
                  <Download className="w-3.5 h-3.5" />
                  导出 Excel (.xlsx)
                </button>
                <button
                  onClick={() => {
                    if (settlementReport?.markdown) {
                      navigator.clipboard.writeText(settlementReport.markdown);
                      showToast("Markdown 结算对账单已复制到剪贴板！");
                    }
                  }}
                  className="px-4 py-2 text-xs font-bold bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg shadow-sm flex items-center gap-1.5 transition-all"
                >
                  <Copy className="w-3.5 h-3.5" />
                  复制 Markdown
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框: 智能批量补录 (Git时间段抓取 / AI智能体报告解析) ---------------- */}
      {showBatchModal && activeProject && (
        <div className="fixed inset-0 bg-slate-900/50 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-150">
          <div className="w-full max-w-4xl bg-white rounded-2xl border border-slate-200/90 shadow-2xl flex flex-col max-h-[90vh] overflow-hidden">
            {/* 模态框顶部 Header */}
            <div className="p-5 border-b border-slate-100 flex items-center justify-between bg-slate-50/60 shrink-0">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-xl bg-indigo-50 border border-indigo-200/80 flex items-center justify-center text-indigo-600 text-lg font-bold shadow-2xs">
                  ✨
                </div>
                <div>
                  <h3 className="text-base font-extrabold text-slate-900 tracking-tight flex items-center gap-2">
                    智能批量补录工时
                    <span className="px-2 py-0.5 rounded text-[11px] font-semibold bg-indigo-100 text-indigo-700">
                      {activeProject.name}
                    </span>
                  </h3>
                  <p className="text-xs text-slate-500 font-medium">
                    按起止时间段自动检索 Git 提交或粘贴 AI 智能体/周报总结，支持人工在线精细化复核与批量入库
                  </p>
                </div>
              </div>
              <button
                onClick={() => setShowBatchModal(false)}
                className="w-8 h-8 rounded-lg flex items-center justify-center text-slate-400 hover:text-slate-700 hover:bg-slate-200/60 transition-colors text-base font-bold cursor-pointer"
              >
                ✕
              </button>
            </div>

            {/* 模态框中间内容区 */}
            <div className="flex-1 overflow-y-auto p-5 space-y-4">
              {/* Tab 切换：按 Git 时间段抓取 vs 粘贴 AI 智能体工作总结 */}
              <div className="flex border-b border-slate-200">
                <button
                  type="button"
                  onClick={() => setBatchTab("git")}
                  className={`pb-2.5 px-4 text-xs font-bold border-b-2 flex items-center gap-1.5 transition-all cursor-pointer ${
                    batchTab === "git"
                      ? "border-indigo-600 text-indigo-600"
                      : "border-transparent text-slate-500 hover:text-slate-800"
                  }`}
                >
                  <GitBranch className="w-3.5 h-3.5" />
                  模式 1：按 Git 版本库起止时间段检索
                </button>
                <button
                  type="button"
                  onClick={() => setBatchTab("text")}
                  className={`pb-2.5 px-4 text-xs font-bold border-b-2 flex items-center gap-1.5 transition-all cursor-pointer ${
                    batchTab === "text"
                      ? "border-indigo-600 text-indigo-600"
                      : "border-transparent text-slate-500 hover:text-slate-800"
                  }`}
                >
                  <Sparkles className="w-3.5 h-3.5" />
                  模式 2：粘贴 AI 智能体/周期工作总结
                </button>
              </div>

              {/* 模式 1：Git 时间段抓取面板 */}
              {batchTab === "git" && (
                <div className="bg-slate-50/70 border border-slate-200/80 rounded-xl p-4 space-y-3">
                  <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                    <div>
                      <label className="text-[11px] font-bold text-slate-600 block mb-1">开始日期</label>
                      <ModernDatePicker
                        value={batchStartDate}
                        onChange={setBatchStartDate}
                        className="w-full h-8 px-2.5 rounded-lg border border-slate-200 bg-white text-xs font-mono"
                      />
                    </div>
                    <div>
                      <label className="text-[11px] font-bold text-slate-600 block mb-1">结束日期</label>
                      <ModernDatePicker
                        value={batchEndDate}
                        onChange={setBatchEndDate}
                        className="w-full h-8 px-2.5 rounded-lg border border-slate-200 bg-white text-xs font-mono"
                      />
                    </div>
                    <div>
                      <label className="text-[11px] font-bold text-slate-600 block mb-1">本地 Git 仓库路径</label>
                      <input
                        type="text"
                        value={batchRepoPath}
                        onChange={(e) => setBatchRepoPath(e.target.value)}
                        placeholder="如: /home/beatle/dev/my-repo"
                        className="w-full h-8 px-2.5 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-mono"
                      />
                    </div>
                  </div>

                  <div className="flex items-center justify-between pt-1">
                    <p className="text-[11px] text-slate-500">
                      💡 提示：系统将扫描该区间所有 Git Commit，自动按自然日归集，并利用高层次工程模型提炼高价值交付清单与上下班工时。
                    </p>
                    <button
                      type="button"
                      onClick={handleBatchScanGit}
                      disabled={isBatchScanning}
                      className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg text-xs font-bold flex items-center gap-1.5 shadow-xs transition-all cursor-pointer disabled:opacity-50 shrink-0"
                    >
                      <Sparkles className={`w-3.5 h-3.5 ${isBatchScanning ? "animate-spin" : ""}`} />
                      {isBatchScanning ? "正在扫描并由 AI 评估..." : "开始检索并提取"}
                    </button>
                  </div>
                </div>
              )}

              {/* 模式 2：粘贴 AI 智能体工作总结自由文本 */}
              {batchTab === "text" && (
                <div className="bg-slate-50/70 border border-slate-200/80 rounded-xl p-4 space-y-3">
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">
                      粘贴 AI 智能体输出的周期总结、周报、任务清单或自由日志文本
                    </label>
                    <textarea
                      value={batchRawText}
                      onChange={(e) => setBatchRawText(e.target.value)}
                      placeholder="例如粘贴：&#10;2026-04-10: 完成年度研发费用规划核心业务模块重构与多维度核算，耗时 09:30-18:30&#10;2026-04-11: 研发预算编制约束机制及Excel校验比例导入引擎开发..."
                      rows={4}
                      className="w-full p-2.5 rounded-xl border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:ring-1 focus:ring-indigo-500 font-mono leading-relaxed"
                    />
                  </div>

                  <div className="flex items-center justify-between">
                    <p className="text-[11px] text-slate-500">
                      🤖 智能体解析引擎将自动识别并抽取所有日期、工作时段，并将长句解析为高大上规范条目。
                    </p>
                    <button
                      type="button"
                      onClick={handleBatchParseText}
                      disabled={isBatchScanning}
                      className="px-4 py-2 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg text-xs font-bold flex items-center gap-1.5 shadow-xs transition-all cursor-pointer disabled:opacity-50 shrink-0"
                    >
                      <Sparkles className={`w-3.5 h-3.5 ${isBatchScanning ? "animate-spin" : ""}`} />
                      {isBatchScanning ? "正在智能解析..." : "AI 解析为工时列表"}
                    </button>
                  </div>
                </div>
              )}

              {/* 候选待确认条目列表 (支持多选、单项自由编辑) */}
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="text-xs font-bold text-slate-800">
                      待补录候选列表 ({batchEntries.length} 天)
                    </span>
                    {batchEntries.length > 0 && (
                      <span className="text-[11px] text-indigo-600 font-semibold bg-indigo-50 px-2 py-0.5 rounded">
                        已选 {batchEntries.filter((e) => e.is_selected).length} 项
                      </span>
                    )}
                  </div>
                  {batchEntries.length > 0 && (
                    <div className="flex items-center gap-2 text-xs">
                      <button
                        type="button"
                        onClick={() =>
                          setBatchEntries((prev) =>
                            prev.map((e) => ({ ...e, is_selected: true }))
                          )
                        }
                        className="text-indigo-600 hover:underline font-bold cursor-pointer"
                      >
                        全选
                      </button>
                      <span className="text-slate-300">|</span>
                      <button
                        type="button"
                        onClick={() =>
                          setBatchEntries((prev) =>
                            prev.map((e) => ({ ...e, is_selected: false }))
                          )
                        }
                        className="text-slate-500 hover:text-slate-700 cursor-pointer"
                      >
                        取消全选
                      </button>
                    </div>
                  )}
                </div>

                {batchEntries.length === 0 ? (
                  <div className="p-8 border border-dashed border-slate-200 rounded-xl text-center text-slate-400 text-xs">
                    暂无候选条目，请在上方输入检索范围或粘贴总结文本后点击提取
                  </div>
                ) : (
                  <div className="space-y-3 max-h-[380px] overflow-y-auto pr-1">
                    {batchEntries.map((entry, idx) => (
                      <div
                        key={entry.temp_id || idx}
                        className={`p-3.5 rounded-xl border transition-all ${
                          entry.is_selected
                            ? "bg-white border-indigo-200 shadow-2xs ring-1 ring-indigo-500/10"
                            : "bg-slate-50/60 border-slate-200/70 opacity-60"
                        }`}
                      >
                        <div className="flex items-center justify-between gap-3 mb-2.5">
                          <div className="flex items-center gap-3">
                            <input
                              type="checkbox"
                              checked={entry.is_selected}
                              onChange={(e) => {
                                const checked = e.target.checked;
                                setBatchEntries((prev) =>
                                  prev.map((it, i) =>
                                    i === idx ? { ...it, is_selected: checked } : it
                                  )
                                );
                              }}
                              className="w-4 h-4 rounded text-indigo-600 cursor-pointer"
                            />
                            <div className="flex items-center gap-2">
                              <span className="text-xs font-bold text-slate-900 font-mono">
                                {entry.date}
                              </span>
                              {entry.commits_count > 0 && (
                                <span className="px-1.5 py-0.2 rounded text-[10px] bg-slate-100 text-slate-600 font-medium">
                                  {entry.commits_count} 次代码提交
                                </span>
                              )}
                              {entry.reason && (
                                <span className="text-[10px] text-indigo-600 font-medium bg-indigo-50 px-1.5 py-0.2 rounded truncate max-w-xs">
                                  💡 {entry.reason}
                                </span>
                              )}
                            </div>
                          </div>

                          <div className="flex items-center gap-2 shrink-0">
                            <div className="flex items-center gap-1">
                              <span className="text-[11px] text-slate-500">时段:</span>
                              <ModernTimePicker
                                value={entry.start_time}
                                onChange={(val) =>
                                  setBatchEntries((prev) =>
                                    prev.map((it, i) =>
                                      i === idx ? { ...it, start_time: val } : it
                                    )
                                  )
                                }
                                className="w-20 h-7 text-xs font-mono rounded border border-slate-200 px-1.5"
                              />
                              <span className="text-slate-400 text-xs">~</span>
                              <ModernTimePicker
                                value={entry.end_time}
                                onChange={(val) =>
                                  setBatchEntries((prev) =>
                                    prev.map((it, i) =>
                                      i === idx ? { ...it, end_time: val } : it
                                    )
                                  )
                                }
                                className="w-20 h-7 text-xs font-mono rounded border border-slate-200 px-1.5"
                              />
                            </div>

                            <button
                              type="button"
                              onClick={() =>
                                setBatchEntries((prev) => prev.filter((_, i) => i !== idx))
                              }
                              className="text-slate-400 hover:text-rose-600 p-1 rounded transition-colors cursor-pointer"
                              title="移除此行"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          </div>
                        </div>

                        {/* 工作内容条目编辑器 */}
                        <div>
                          <textarea
                            value={entry.summary}
                            onChange={(e) => {
                              const val = e.target.value;
                              setBatchEntries((prev) =>
                                prev.map((it, i) =>
                                  i === idx ? { ...it, summary: val } : it
                                )
                              );
                            }}
                            rows={3}
                            placeholder="工作成果交付条目..."
                            className="w-full p-2 text-xs rounded-lg border border-slate-200 bg-white focus:outline-none focus:ring-1 focus:ring-indigo-500 font-mono leading-relaxed"
                          />
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>

            {/* 模态框底部操作 Footer */}
            <div className="p-4 border-t border-slate-100 flex items-center justify-between bg-slate-50/50 shrink-0">
              <div className="text-xs text-slate-500">
                当前选中待入库:{" "}
                <strong className="text-indigo-600 font-bold">
                  {batchEntries.filter((e) => e.is_selected).length}
                </strong>{" "}
                / {batchEntries.length} 条记录
              </div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => setShowBatchModal(false)}
                  className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors cursor-pointer"
                >
                  取消
                </button>
                <button
                  type="button"
                  disabled={
                    isBatchSaving ||
                    batchEntries.filter((e) => e.is_selected).length === 0
                  }
                  onClick={handleBatchSave}
                  className="px-5 py-2 text-xs font-bold bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg shadow-sm flex items-center gap-1.5 transition-all cursor-pointer disabled:opacity-50"
                >
                  <CheckCircle2 className="w-3.5 h-3.5" />
                  {isBatchSaving
                    ? "正在批量入库中..."
                    : `确认批量入库 (${batchEntries.filter((e) => e.is_selected).length} 条)`}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框: 会议纪要确认与编辑 (单独结束会议产生) ---------------- */}
      {showMeetingSummaryModal && meetingSummaryData && (
        <div className="fixed inset-0 bg-slate-900/50 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-150">
          <div className="w-full max-w-2xl bg-white rounded-2xl border border-slate-200/90 shadow-2xl flex flex-col max-h-[90vh] overflow-hidden">
            {/* 模态框顶部 Header */}
            <div className="p-5 border-b border-slate-100 flex items-center justify-between bg-slate-50/60 shrink-0">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-xl bg-indigo-50 border border-indigo-200/80 flex items-center justify-center text-indigo-600 text-lg font-bold shadow-2xs">
                  🎙️
                </div>
                <div>
                  <h3 className="text-base font-extrabold text-slate-900 tracking-tight flex items-center gap-2">
                    会议纪要提炼与编辑
                    <span className="px-2 py-0.5 rounded text-[11px] font-semibold bg-indigo-100 text-indigo-700">
                      {meetingSummaryData.topic}
                    </span>
                  </h3>
                  <p className="text-xs text-slate-500 font-medium">
                    本次会议录音已安全归档落盘，项目主工时保持继续计时中。您可以对 AI 生成的纪要进行修改与补充后保存。
                  </p>
                </div>
              </div>
              <button
                onClick={() => setShowMeetingSummaryModal(false)}
                className="w-8 h-8 rounded-lg flex items-center justify-center text-slate-400 hover:text-slate-700 hover:bg-slate-200/60 transition-colors text-base font-bold cursor-pointer"
              >
                ✕
              </button>
            </div>

            {/* 模态框中间编辑区 */}
            <div className="flex-1 overflow-y-auto p-5 space-y-4">
              {/* 录音与凭证信息提示 */}
              <div className="p-3 bg-indigo-50/60 border border-indigo-100 rounded-xl text-xs text-indigo-900 flex items-center justify-between">
                <div className="flex items-center gap-2 truncate">
                  <span className="text-base">📁</span>
                  <span className="font-semibold text-slate-700">高保真录音已存盘:</span>
                  <span className="font-mono text-slate-500 truncate max-w-sm" title={meetingSummaryData.audio_path}>
                    {meetingSummaryData.audio_path}
                  </span>
                </div>
                {meetingSummaryData.audio_path && (
                  <button
                    type="button"
                    onClick={() => handleOpenAudioPlayer(meetingSummaryData.audio_path, `【${meetingSummaryData.topic}】会议录音`)}
                    className="px-2.5 py-1 bg-white hover:bg-indigo-100 border border-indigo-200 text-indigo-700 rounded-md font-bold text-[11px] shrink-0 cursor-pointer shadow-2xs flex items-center gap-1"
                  >
                    <Volume2 className="w-3.5 h-3.5 text-indigo-600" />
                    试听录音
                  </button>
                )}
              </div>

              {/* 会议主题输入修改 */}
              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">
                  会议主题 / 议题
                </label>
                <input
                  type="text"
                  value={meetingSummaryData.topic}
                  onChange={(e) =>
                    setMeetingSummaryData((prev) =>
                      prev ? { ...prev, topic: e.target.value } : prev
                    )
                  }
                  className="w-full h-9 px-3 rounded-xl border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-1 focus:ring-indigo-500 font-medium"
                />
              </div>

              {/* 结构化会议纪要多行编辑 */}
              <div>
                <div className="flex items-center justify-between mb-1">
                  <label className="text-xs font-bold text-slate-700 flex items-center gap-1.5">
                    <span>✨</span> AI 生成的会议纪要 (可直接编辑修改)
                  </label>
                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      onClick={() => {
                        if (meetingSummaryData.summary) {
                          navigator.clipboard.writeText(meetingSummaryData.summary);
                          showToast("会议纪要已复制到剪贴板！");
                        }
                      }}
                      className="text-[11px] text-indigo-600 hover:underline font-semibold cursor-pointer flex items-center gap-1"
                    >
                      <Copy className="w-3 h-3" />
                      复制纪要
                    </button>
                    <button
                      type="button"
                      disabled={isExportingPdf}
                      onClick={() => {
                        const proj = projects.find((p) => p.id === meetingSummaryData.project_id);
                        handleExportPdf({
                          title: `【${meetingSummaryData.topic}】会议纪要与技术决议`,
                          projectName: proj ? proj.name : "项目沟通",
                          dateStr: new Date().toISOString().split("T")[0],
                          summaryContent: meetingSummaryData.summary,
                        });
                      }}
                      className="text-[11px] text-emerald-600 hover:underline font-semibold cursor-pointer flex items-center gap-1 disabled:opacity-50"
                    >
                      <FileDown className="w-3 h-3 text-emerald-600" />
                      导出PDF
                    </button>
                  </div>
                </div>
                <textarea
                  value={meetingSummaryData.summary}
                  onChange={(e) =>
                    setMeetingSummaryData((prev) =>
                      prev ? { ...prev, summary: e.target.value } : prev
                    )
                  }
                  rows={8}
                  placeholder="编辑会议纪要内容..."
                  className="w-full p-3 rounded-xl border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-1 focus:ring-indigo-500 font-mono leading-relaxed"
                />
              </div>

              {/* 语音转写原始文稿折叠展示 */}
              {meetingSummaryData.transcript && (
                <div className="border border-slate-200 rounded-xl overflow-hidden">
                  <details className="text-xs">
                    <summary className="p-3 bg-slate-50 font-bold text-slate-700 cursor-pointer select-none flex items-center justify-between">
                      <span>📝 查看语音转写完整实录文本 (展开)</span>
                      <span className="text-[11px] text-slate-400 font-normal">点击展开/收起</span>
                    </summary>
                    <div className="p-3.5 bg-white border-t border-slate-100 max-h-48 overflow-y-auto font-mono text-[11px] text-slate-600 leading-relaxed whitespace-pre-wrap">
                      {meetingSummaryData.transcript}
                    </div>
                  </details>
                </div>
              )}
            </div>

            {/* 模态框底部操作 Footer */}
            <div className="p-4 border-t border-slate-100 flex items-center justify-between bg-slate-50/50 shrink-0">
              <span className="text-xs text-slate-500">
                保存后将自动沉淀为该项目的交付备忘与讨论凭据
              </span>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => setShowMeetingSummaryModal(false)}
                  className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors cursor-pointer"
                >
                  暂存/关闭
                </button>
                <button
                  type="button"
                  onClick={handleSaveMeetingSummary}
                  className="px-5 py-2 text-xs font-bold bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg shadow-sm flex items-center gap-1.5 transition-all cursor-pointer"
                >
                  <CheckCircle2 className="w-3.5 h-3.5" />
                  确认并存入项目档案
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 8: 专业 Excel 结算对账单导出配置 ---------------- */}
      {showExportSettingsModal && (
        <div className="fixed inset-0 bg-slate-900/50 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-150">
          <div className="w-full max-w-2xl bg-white rounded-2xl border border-slate-200/90 shadow-2xl p-6 space-y-5">
            {/* 标题 */}
            <div className="flex items-center justify-between pb-3 border-b border-slate-100">
              <div className="flex items-center gap-2.5">
                <div className="w-9 h-9 rounded-xl bg-emerald-50 border border-emerald-200/80 flex items-center justify-center text-emerald-600 text-lg font-bold">
                  📥
                </div>
                <div>
                  <h3 className="text-base font-extrabold text-slate-900 tracking-tight">
                    导出项目人天对账单 (Excel)
                  </h3>
                  <p className="text-xs text-slate-500 font-medium">
                    每个项目生成独立 .xlsx 文件 · 各月份作为独立工作表 (Sheet)
                  </p>
                </div>
              </div>
              <button
                onClick={() => setShowExportSettingsModal(false)}
                className="w-7 h-7 flex items-center justify-center text-slate-400 hover:text-slate-700 hover:bg-slate-100 rounded-lg text-sm font-bold transition-colors cursor-pointer"
              >
                ✕
              </button>
            </div>

            {/* 1. 核算结算策略选择 (核心单选卡片) */}
            <div className="space-y-2">
              <label className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
                <span>⚖️</span> 选择人天核算与流动策略 (必选)
              </label>
              <div className="grid grid-cols-2 gap-2.5">
                {/* 策略 1: 削峰填谷 */}
                <div
                  onClick={() => setExportStrategy("rebalance")}
                  className={`p-3 rounded-xl border text-left cursor-pointer transition-all ${
                    exportStrategy === "rebalance"
                      ? "bg-indigo-50/60 border-indigo-500 ring-2 ring-indigo-500/20"
                      : "bg-white hover:bg-slate-50 border-slate-200/80"
                  }`}
                >
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-xs font-extrabold text-slate-900 flex items-center gap-1.5">
                      <span>🛡️</span> 合规削峰填谷
                    </span>
                    <span className="text-[10px] px-1.5 py-0.2 rounded bg-indigo-100 text-indigo-700 font-bold">标准推荐</span>
                  </div>
                  <p className="text-[11px] text-slate-500 leading-relaxed">
                    工作日单日封顶 8 小时 (1.0天)；溢出工时回填历史不足工作日或顺延未来工作日，周末不排工时。
                  </p>
                </div>

                {/* 策略 2: 保底 8 小时 */}
                <div
                  onClick={() => setExportStrategy("min8_carry")}
                  className={`p-3 rounded-xl border text-left cursor-pointer transition-all ${
                    exportStrategy === "min8_carry"
                      ? "bg-indigo-50/60 border-indigo-500 ring-2 ring-indigo-500/20"
                      : "bg-white hover:bg-slate-50 border-slate-200/80"
                  }`}
                >
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-xs font-extrabold text-slate-900 flex items-center gap-1.5">
                      <span>💎</span> 不足8h按8h (超额顺延)
                    </span>
                    <span className="text-[10px] px-1.5 py-0.2 rounded bg-amber-100 text-amber-800 font-bold">顾问/驻场</span>
                  </div>
                  <p className="text-[11px] text-slate-500 leading-relaxed">
                    只要当天有效投入，基准按 1.0 人天起步保底；超过 8 小时部分自动结转为新工作日追加。
                  </p>
                </div>

                {/* 策略 3: 单日封顶 8h，超额直接顺延新天 */}
                <div
                  onClick={() => setExportStrategy("cap8_carry")}
                  className={`p-3 rounded-xl border text-left cursor-pointer transition-all ${
                    exportStrategy === "cap8_carry"
                      ? "bg-indigo-50/60 border-indigo-500 ring-2 ring-indigo-500/20"
                      : "bg-white hover:bg-slate-50 border-slate-200/80"
                  }`}
                >
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-xs font-extrabold text-slate-900 flex items-center gap-1.5">
                      <span>⏩</span> 封顶顺延新天
                    </span>
                    <span className="text-[10px] px-1.5 py-0.2 rounded bg-slate-100 text-slate-600 font-bold">按日延期</span>
                  </div>
                  <p className="text-[11px] text-slate-500 leading-relaxed">
                    当天实报实销但不超过 8 小时；超出 8 小时的溢出工时直接结转出新的工作日追加在后。
                  </p>
                </div>

                {/* 策略 4: 严格实际工时合并 */}
                <div
                  onClick={() => setExportStrategy("exact")}
                  className={`p-3 rounded-xl border text-left cursor-pointer transition-all ${
                    exportStrategy === "exact"
                      ? "bg-indigo-50/60 border-indigo-500 ring-2 ring-indigo-500/20"
                      : "bg-white hover:bg-slate-50 border-slate-200/80"
                  }`}
                >
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-xs font-extrabold text-slate-900 flex items-center gap-1.5">
                      <span>⏱️</span> 实际物理工时合并
                    </span>
                    <span className="text-[10px] px-1.5 py-0.2 rounded bg-slate-100 text-slate-600 font-bold">实报实销</span>
                  </div>
                  <p className="text-[11px] text-slate-500 leading-relaxed">
                    同一天同项目多次打卡合并为唯一一行，按当天实际物理投入比例换算人天，不顺延、不平摊。
                  </p>
                </div>
              </div>
            </div>

            {/* 2. 项目勾选多选 (每个项目一个独立文件) */}
            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <label className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
                  <span>📁</span> 选择要导出的项目 (每个勾选项将生成一个独立 .xlsx 文件)
                </label>
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => setExportSelectedProjects(projects.map(p => p.id))}
                    className="text-[11px] text-indigo-600 hover:underline font-bold cursor-pointer"
                  >
                    全选
                  </button>
                  <span className="text-slate-300">|</span>
                  <button
                    onClick={() => setExportSelectedProjects([])}
                    className="text-[11px] text-slate-400 hover:text-slate-600 font-medium cursor-pointer"
                  >
                    清空
                  </button>
                </div>
              </div>
              <div className="max-h-32 overflow-y-auto p-2 bg-slate-50 border border-slate-200/80 rounded-xl grid grid-cols-2 gap-2">
                {projects.map((p) => {
                  const isChecked = exportSelectedProjects.includes(p.id);
                  return (
                    <label
                      key={p.id}
                      className={`flex items-center gap-2 p-2 rounded-lg border text-xs cursor-pointer transition-all ${
                        isChecked
                          ? "bg-white border-indigo-300 text-slate-900 shadow-2xs font-semibold"
                          : "border-transparent text-slate-600 hover:bg-slate-100/70"
                      }`}
                    >
                      <input
                        type="checkbox"
                        checked={isChecked}
                        onChange={(e) => {
                          if (e.target.checked) {
                            setExportSelectedProjects([...exportSelectedProjects, p.id]);
                          } else {
                            setExportSelectedProjects(exportSelectedProjects.filter((id) => id !== p.id));
                          }
                        }}
                        className="rounded text-indigo-600"
                      />
                      <span className="truncate flex-1">{p.name}</span>
                    </label>
                  );
                })}
              </div>
            </div>

            {/* 3. 月份勾选多选 (每个月份作为独立 Sheet) */}
            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <label className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
                  <span>📅</span> 选择导出的月份 (在项目文件内按月拆分 Sheet 工作表)
                </label>
              </div>
              <div className="flex flex-wrap gap-2">
                {[
                  currentMonthStr,
                  ...Array.from({ length: 5 }).map((_, i) => {
                    const d = new Date(now.getFullYear(), now.getMonth() - (i + 1), 1);
                    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
                  }),
                ].map((mStr) => {
                  const isChecked = exportSelectedMonths.includes(mStr);
                  const [y, m] = mStr.split("-");
                  return (
                    <button
                      key={mStr}
                      onClick={() => {
                        if (isChecked) {
                          if (exportSelectedMonths.length > 1) {
                            setExportSelectedMonths(exportSelectedMonths.filter((x) => x !== mStr));
                          } else {
                            showToast("至少需保留一个月份！");
                          }
                        } else {
                          setExportSelectedMonths([...exportSelectedMonths, mStr]);
                        }
                      }}
                      className={`px-3 py-1.5 rounded-lg text-xs font-bold border transition-all cursor-pointer ${
                        isChecked
                          ? "bg-indigo-600 text-white border-indigo-600 shadow-2xs"
                          : "bg-white text-slate-600 border-slate-200 hover:bg-slate-50"
                      }`}
                    >
                      {`${y}年${m}月`}
                    </button>
                  );
                })}
              </div>
            </div>

            {/* 4. 导出字段与金额显示配置 (新增选项) */}
            <div className="p-3 bg-slate-50 border border-slate-200/80 rounded-xl flex items-center justify-between">
              <div>
                <span className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
                  <span>💰</span> 导出包含结算金额与单价信息
                </span>
                <p className="text-[11px] text-slate-500 mt-0.5">
                  关闭后，Excel 中将直接剔除「核算金额」列及金额汇总，仅保留投入工时、人天折算与交付成果
                </p>
              </div>
              <label className="relative inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  checked={exportIncludeAmount}
                  onChange={(e) => setExportIncludeAmount(e.target.checked)}
                  className="sr-only peer"
                />
                <div className="w-9 h-5 bg-slate-300 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-slate-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-emerald-600"></div>
                <span className="ml-2 text-xs font-bold text-slate-700">
                  {exportIncludeAmount ? "包含金额" : "隐藏金额"}
                </span>
              </label>
            </div>

            {/* 底部按钮栏 */}
            <div className="flex items-center justify-between pt-3 border-t border-slate-100">
              <div className="text-[11px] text-slate-500">
                将生成 <span className="font-bold text-slate-900">{exportSelectedProjects.length || projects.length}</span> 个独立文件，
                每个文件内含 <span className="font-bold text-slate-900">{exportSelectedMonths.length}</span> 个月份工作表
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setShowExportSettingsModal(false)}
                  className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors cursor-pointer"
                >
                  取消
                </button>
                <button
                  disabled={isExportingExcel}
                  onClick={executeExportExcel}
                  className="px-5 py-2 text-xs font-bold bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg shadow-sm flex items-center gap-1.5 transition-all cursor-pointer disabled:opacity-50"
                >
                  <Download className="w-3.5 h-3.5" />
                  {isExportingExcel ? "正在生成..." : "确认导出"}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框: 专业内置音频播放器 (支持拖动进度条、播放/暂停、复制路径、定位目录) ---------------- */}
      {activePlayerAudio && (
        <div className="fixed inset-0 bg-slate-900/50 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-150">
          <div className="w-full max-w-lg bg-white rounded-2xl border border-slate-200/90 shadow-2xl p-6 space-y-4">
            {/* 顶栏 */}
            <div className="flex items-center justify-between pb-3 border-b border-slate-100">
              <div className="flex items-center gap-2.5">
                <div className="w-9 h-9 rounded-xl bg-indigo-50 border border-indigo-200/80 flex items-center justify-center text-indigo-600 text-lg">
                  🔊
                </div>
                <div>
                  <h3 className="text-sm font-extrabold text-slate-900 tracking-tight">
                    {activePlayerAudio.title}
                  </h3>
                  <p className="text-[11px] text-slate-500 font-medium">
                    内置音频播放器 · 支持滚动条精准拖动与定位
                  </p>
                </div>
              </div>
              <button
                onClick={() => setActivePlayerAudio(null)}
                className="w-7 h-7 flex items-center justify-center text-slate-400 hover:text-slate-700 hover:bg-slate-100 rounded-lg text-sm font-bold transition-colors cursor-pointer"
              >
                ✕
              </button>
            </div>

            {/* 音频播放控件或加载状态 */}
            <div className="bg-slate-50/80 border border-slate-200/80 rounded-xl p-4 space-y-3">
              {activePlayerAudio.loading ? (
                <div className="flex items-center justify-center py-4 text-xs font-semibold text-slate-500 gap-2">
                  <span className="w-3.5 h-3.5 border-2 border-indigo-600 border-t-transparent rounded-full animate-spin" />
                  正在加载音频数据流...
                </div>
              ) : activePlayerAudio.dataUrl ? (
                <audio
                  controls
                  autoPlay
                  src={activePlayerAudio.dataUrl}
                  className="w-full h-11 focus:outline-none rounded-lg"
                />
              ) : (
                <div className="text-xs text-rose-500 text-center py-2 font-medium">
                  音频文件未就绪或无法解析播放
                </div>
              )}
            </div>

            {/* 路径详情与复制/定位功能 */}
            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <span className="text-[11px] font-bold text-slate-700">录音文件绝对路径</span>
                <span className="text-[10px] text-slate-400 font-mono">本地文件凭证</span>
              </div>
              <div className="flex items-center gap-2">
                <div
                  className="flex-1 px-3 py-2 bg-slate-100 rounded-lg text-[11px] font-mono text-slate-700 truncate border border-slate-200/80 select-all"
                  title={activePlayerAudio.path}
                >
                  {activePlayerAudio.path}
                </div>
                <button
                  type="button"
                  onClick={() => {
                    navigator.clipboard.writeText(activePlayerAudio.path);
                    showToast("📋 录音完整路径已复制到剪贴板！");
                  }}
                  className="h-8 px-3 rounded-lg bg-indigo-50 hover:bg-indigo-100 border border-indigo-200 text-indigo-700 text-xs font-bold flex items-center gap-1.5 shadow-2xs transition-all cursor-pointer shrink-0"
                  title="一键复制文件完整路径"
                >
                  <Copy className="w-3.5 h-3.5" />
                  复制路径
                </button>
                <button
                  type="button"
                  onClick={() => {
                    invoke("open_file_or_dir", { path: activePlayerAudio.path });
                    showToast("📂 已在系统文件管理器中定位该录音！");
                  }}
                  className="h-8 px-3 rounded-lg bg-slate-100 hover:bg-slate-200 border border-slate-200 text-slate-700 text-xs font-semibold flex items-center gap-1.5 shadow-2xs transition-all cursor-pointer shrink-0"
                  title="在系统文件管理器中打开"
                >
                  <FolderOpen className="w-3.5 h-3.5 text-slate-600" />
                  打开目录
                </button>
              </div>
            </div>

            {/* 底部按钮 */}
            <div className="flex items-center justify-end pt-2 border-t border-slate-100">
              <button
                type="button"
                onClick={() => setActivePlayerAudio(null)}
                className="px-4 py-2 text-xs font-bold bg-slate-100 hover:bg-slate-200 text-slate-700 rounded-lg transition-colors cursor-pointer"
              >
                关闭播放器
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框: 独立会议逐字全量转写文稿查看器 ---------------- */}
      {activeTranscriptModal && (
        <div className="fixed inset-0 bg-slate-900/50 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-150">
          <div className="w-full max-w-3xl bg-white rounded-2xl border border-slate-200/90 shadow-2xl flex flex-col max-h-[85vh] overflow-hidden">
            {/* Header */}
            <div className="p-4 border-b border-slate-100 flex items-center justify-between bg-slate-50/60 shrink-0">
              <div className="flex items-center gap-2.5">
                <div className="w-8 h-8 rounded-lg bg-indigo-50 border border-indigo-200/80 flex items-center justify-center text-indigo-600 text-sm font-bold">
                  📝
                </div>
                <div>
                  <h3 className="text-sm font-extrabold text-slate-900 tracking-tight">
                    {activeTranscriptModal.title}
                  </h3>
                  <p className="text-[11px] text-slate-400 font-medium">
                    原始录音逐字转写全量文稿（共 {activeTranscriptModal.transcript.length} 字）
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => {
                    navigator.clipboard.writeText(activeTranscriptModal.transcript);
                    showToast("📋 转写原文已复制到剪贴板！");
                  }}
                  className="px-2.5 py-1.5 rounded-lg bg-indigo-50 hover:bg-indigo-100 border border-indigo-200 text-indigo-700 text-xs font-bold flex items-center gap-1 transition-all cursor-pointer shadow-2xs"
                >
                  <Copy className="w-3.5 h-3.5" />
                  复制全文
                </button>
                <button
                  onClick={() => setActiveTranscriptModal(null)}
                  className="w-7 h-7 flex items-center justify-center text-slate-400 hover:text-slate-700 hover:bg-slate-200/60 rounded-lg text-sm font-bold transition-colors cursor-pointer"
                >
                  ✕
                </button>
              </div>
            </div>

            {/* Content */}
            <div className="flex-1 overflow-y-auto p-5 font-mono text-xs text-slate-700 leading-relaxed bg-slate-50/30 whitespace-pre-wrap select-text">
              {activeTranscriptModal.transcript}
            </div>

            {/* Footer */}
            <div className="p-3 border-t border-slate-100 flex items-center justify-between bg-slate-50/50 shrink-0 text-xs text-slate-400">
              <span>可复制后粘贴至专业文档或留存归档</span>
              <button
                onClick={() => setActiveTranscriptModal(null)}
                className="px-4 py-1.5 bg-slate-200 hover:bg-slate-300 text-slate-700 font-bold rounded-lg transition-colors cursor-pointer"
              >
                关闭
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框: 会议纪要在线编辑与修改 (为丢给 Agent 开发做准备) ---------------- */}
      {activeEditSummaryModal && (
        <div className="fixed inset-0 bg-slate-900/50 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-150">
          <div className="w-full max-w-3xl bg-white rounded-2xl border border-slate-200/90 shadow-2xl flex flex-col max-h-[90vh] overflow-hidden">
            {/* Header */}
            <div className="p-4 border-b border-slate-100 flex items-center justify-between bg-slate-50/60 shrink-0">
              <div className="flex items-center gap-2.5">
                <div className="w-8 h-8 rounded-lg bg-indigo-50 border border-indigo-200/80 flex items-center justify-center text-indigo-600 text-sm font-bold">
                  ✍️
                </div>
                <div>
                  <h3 className="text-sm font-extrabold text-slate-900 tracking-tight">
                    {activeEditSummaryModal.title}
                  </h3>
                  <p className="text-[11px] text-slate-500 font-medium">
                    人工校对与完善要点 · 保存后一键复制丢给 Agent 进行工程开发
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => {
                    navigator.clipboard.writeText(activeEditSummaryModal.summary);
                    showToast("📋 编辑中文本已复制！可直接发送给 AI 智能体");
                  }}
                  className="px-2.5 py-1.5 rounded-lg bg-indigo-50 hover:bg-indigo-100 border border-indigo-200 text-indigo-700 text-xs font-bold flex items-center gap-1 transition-all cursor-pointer shadow-2xs"
                  title="一键复制当前文本"
                >
                  <Copy className="w-3.5 h-3.5" />
                  复制纪要
                </button>
                <button
                  type="button"
                  disabled={isExportingPdf}
                  onClick={() => handleExportPdf({
                    title: activeEditSummaryModal.title,
                    projectName: activeEditSummaryModal.project_name || "项目交付",
                    dateStr: activeEditSummaryModal.date_str || new Date().toISOString().split("T")[0],
                    summaryContent: activeEditSummaryModal.summary,
                  })}
                  className="px-2.5 py-1.5 rounded-lg bg-emerald-50 hover:bg-emerald-100 border border-emerald-200 text-emerald-700 text-xs font-bold flex items-center gap-1 transition-all cursor-pointer shadow-2xs disabled:opacity-50"
                  title="将当前修改后的会议纪要导出为高端商务交付 PDF"
                >
                  <FileDown className="w-3.5 h-3.5 text-emerald-600" />
                  导出PDF
                </button>
                <button
                  onClick={() => setActiveEditSummaryModal(null)}
                  className="w-7 h-7 flex items-center justify-center text-slate-400 hover:text-slate-700 hover:bg-slate-200/60 rounded-lg text-sm font-bold transition-colors cursor-pointer"
                >
                  ✕
                </button>
              </div>
            </div>

            {/* 编辑主体 */}
            <div className="flex-1 overflow-y-auto p-5 space-y-3 bg-white">
              <div className="flex items-center justify-between pb-1">
                <span className="text-[11px] text-slate-500 font-medium">
                  支持 Markdown 结构排版，随时增删调整业务细节：
                </span>
                <div className="flex items-center gap-2">
                  <span className="font-mono text-xs text-slate-400 mr-1">
                    {activeEditSummaryModal.summary.length} 字
                  </span>
                  <button
                    type="button"
                    disabled={isExtractingGit}
                    onClick={() => handleExtractGitSummary("append")}
                    className="px-2.5 py-1 rounded-lg bg-indigo-50 hover:bg-indigo-100 border border-indigo-200 text-indigo-700 text-xs font-bold flex items-center gap-1.5 transition-all cursor-pointer shadow-2xs disabled:opacity-50"
                    title="从本地 Git 仓库扫描该工时窗口内的提交并追加到末尾"
                  >
                    <RefreshCw className={`w-3.5 h-3.5 text-indigo-600 ${isExtractingGit ? "animate-spin" : ""}`} />
                    追加 Git 记录
                  </button>
                  <button
                    type="button"
                    disabled={isExtractingGit}
                    onClick={() => handleExtractGitSummary("replace")}
                    className="px-2.5 py-1 rounded-lg bg-slate-100 hover:bg-slate-200 text-slate-700 text-xs font-bold flex items-center gap-1.5 transition-all cursor-pointer shadow-2xs disabled:opacity-50"
                    title="从本地 Git 仓库全量重新抽取提交记录并替换当前框内文本"
                  >
                    <GitBranch className="w-3.5 h-3.5 text-slate-600" />
                    全量重新抽取
                  </button>
                </div>
              </div>
              <textarea
                value={activeEditSummaryModal.summary}
                onChange={(e) => setActiveEditSummaryModal({
                  ...activeEditSummaryModal,
                  summary: e.target.value,
                })}
                rows={16}
                className="w-full p-3.5 rounded-xl border border-slate-200 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500 font-mono text-xs text-slate-800 leading-relaxed resize-none bg-slate-50/30"
                placeholder="在此编辑修改会议纪要要点..."
              />
            </div>

            {/* Footer */}
            <div className="p-3.5 border-t border-slate-100 flex items-center justify-between bg-slate-50/50 shrink-0 text-xs">
              <span className="text-slate-400 text-[11px]">
                保存将自动同步更新工时流水与交付档案（完整转写实录不受影响）
              </span>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => setActiveEditSummaryModal(null)}
                  className="px-4 py-2 bg-slate-100 hover:bg-slate-200 text-slate-600 font-bold rounded-lg transition-colors cursor-pointer"
                >
                  取消
                </button>
                <button
                  type="button"
                  onClick={handleSaveEntrySummary}
                  className="px-5 py-2 bg-indigo-600 hover:bg-indigo-700 text-white font-bold rounded-lg shadow-sm flex items-center gap-1.5 transition-all cursor-pointer"
                >
                  <CheckCircle2 className="w-3.5 h-3.5" />
                  保存修改
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 底部轻量 Toast 提示 (带操作按钮) ---------------- */}
      {toast && (
        <div className="fixed bottom-6 left-1/2 -translate-x-1/2 bg-slate-900/95 text-white px-5 py-2.5 rounded-2xl text-xs font-bold shadow-2xl flex items-center gap-3 animate-in fade-in slide-in-from-bottom-2 duration-150 z-50 border border-slate-700/80">
          <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
          <span className="tracking-wide">{toast.message}</span>
          {toast.actionLabel && toast.onAction && (
            <button
              onClick={toast.onAction}
              className="ml-2 px-3 py-1 bg-emerald-500 hover:bg-emerald-600 text-slate-950 font-extrabold rounded-lg text-[11px] shadow-xs transition-colors cursor-pointer flex items-center gap-1 shrink-0"
            >
              {toast.actionLabel}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export default App;
