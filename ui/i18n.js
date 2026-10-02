/**
 * blewred - Preemptive Live Stream Protection Suite
 * Scalable Multilingual Internationalization (i18n) Engine
 * Supported languages: ru (Russian), en (English)
 * Scalability: Add new language dictionaries into DICTIONARIES and append code to SUPPORTED_LANGUAGES.
 */

(function (root, factory) {
  if (typeof define === 'function' && define.amd) {
    define([], factory);
  } else if (typeof module === 'object' && module.exports) {
    module.exports = factory();
  } else {
    root.I18N = factory();
  }
}(typeof self !== 'undefined' ? self : this, function () {
  'use strict';

  const STORAGE_KEY = 'blewred_lang';
  const DEFAULT_LANG = 'en';
  const SUPPORTED_LANGUAGES = ['ru', 'en'];

  const DICTIONARIES = {
    ru: {
      // Header Navigation & Status Ribbon
      "nav_obs": "OBS Studio",
      "nav_obs_plugin": "Плагин OBS",
      "nav_screen": "Видеозахват",
      "nav_shield": "Защита OBS",
      "nav_extension": "Расширение",
      "nav_vision": "Видеоанализ",
      "status_off": "OFF",
      "status_live": "LIVE",
      "status_active": "Активно",
      "status_waiting": "Ожидание...",
      "status_paused": "Пауза",
      "status_scan_idle": "Idle Scan",
      "status_scan_dynamic": "Dynamic 60 FPS",

      // Streamer Setup Guide
      "setup_guide_title": "Быстрый старт стримера: Рекомендуемые шаги настройки",
      "setup_guide_progress": "Готовность: {ready} из {total}",
      "setup_step1_title": "Аппаратное ускорение GPU (DirectML)",
      "setup_step1_desc": "Использование DirectML (DirectX 12) для мгновенного инференса NSFW и OCR нейросетей.",
      "setup_step1_cuda_active": "DirectML активен",
      "setup_step1_cpu_mode": "CPU режим",
      "setup_step2_title": "Локальные нейросетевые модели",
      "setup_step2_desc": "Веса моделей NSFW детекции и OCR для распознавания стоп-слов.",
      "setup_step2_btn": "Скачать модели",
      "setup_step3_title": "Расширение для браузера",
      "setup_step3_desc": "Передает точные тайминги плеера и метаданные видео напрямую в движок.",
      "setup_step3_btn": "Инструкция по установке",
      "setup_step4_title": "Настройка источника OBS Studio",
      "setup_step4_desc": "Добавление источника «Браузер» с Censor Shield для автоматической цензуры.",
      "setup_step4_btn": "Автонастройка OBS",
      "setup_dont_show": "Больше не показывать этот чеклист при запуске",

      // Toolbar Controls
      "toolbar_cues_mgr": "Тайминги плеера",
      "toolbar_stopwords": "Стоп-слова (OCR)",
      "toolbar_nsfw_test": "Тест NSFW",
      "toolbar_ocr_label": "OCR детекция:",
      "toolbar_hotkeys": "Горячие клавиши",

      // Stream & Censor Control Panel
      "stream_control_title": "Контроль видеопотока и цензуры",
      "stream_guard_active": "ЗАЩИТА АКТИВНА",
      "stream_guard_disabled": "ЗАЩИТА ВЫКЛЮЧЕНА",
      "mode_player_title": "Превентивный (Плеер)",
      "mode_player_desc": "Анализ только в окне видеоплеера по опережающим таймингам расширения",
      "mode_screen_title": "Полный экран",
      "mode_screen_desc": "Непрерывный мониторинг всего выбранного дисплея (игры, рабочий стол, чат)",
      "mode_hybrid_title": "Гибридный",
      "mode_hybrid_desc": "Умный фокус: приоритет плееру при воспроизведении, контроль экрана в паузе",
      "mode_off_title": "Защита выкл",
      "mode_off_desc": "Детекция и цензура отключены. Видеопоток передается без анализа.",
      "stream_monitor_label": "Монитор захвата:",
      "stream_shield_label": "Защитный экран (Censor Shield):",
      "stream_shield_desc": "Показывает заглушку в OBS при нарушении",
      "stream_donate_label": "Баннер поддержки разработки:",
      "stream_donate_desc": "Ссылка для поддержки проекта",
      "stream_fps_label": "Частота сканирования (Idle):",
      "stream_fps_5": "5 FPS (Энергосбережение)",
      "stream_fps_60": "60 FPS (Высокая плавность)",

      // Telemetry Cards
      "telemetry_title": "Телеметрия системы и нейросетей",
      "telemetry_hw_accel": "Аппаратное ускорение",
      "telemetry_gpu_card": "GPU & DirectML Инференс",
      "telemetry_scan_speed": "Скорость сканирования",
      "telemetry_latency": "Задержка движка",
      "telemetry_active_rules": "Правила стоп-слов",
      "telemetry_rules_suffix": "активных правил",
      "telemetry_buffer": "Буфер задержки",
      "telemetry_audio_mute": "Аудио цензура",
      "telemetry_audio_normal": "NORMAL",
      "telemetry_audio_muted": "MUTED",

      // Lookahead Warning HUD Panel
      "lookahead_title": "Превентивный мониторинг (Плеер)",
      "lookahead_badge_detected": "ОПАСНЫЙ МОМЕНТ ОБНАРУЖЕН",
      "lookahead_threat_in": "Обнаружена угроза через {sec} сек",
      "lookahead_btn_block": "Заблокировать сейчас",
      "lookahead_btn_allow": "Пропустить (Безопасно)",
      "lookahead_unblurred_tag": "Предосмотр без цензуры",
      "lookahead_cue_timing_label": "Запланированный тайминг",

      // Incident & Audit Log Panel
      "audit_title": "Журнал инцидентов и аудита",
      "audit_stat_total": "Всего:",
      "audit_stat_active": "Активных:",
      "audit_stat_nsfw": "NSFW:",
      "audit_stat_ocr": "OCR:",
      "audit_stat_cues": "Тайминги:",
      "audit_filter_all": "Все",
      "audit_filter_nsfw": "NSFW",
      "audit_filter_ocr": "Стоп-слова",
      "audit_filter_cues": "Тайминги",
      "audit_th_time": "Время",
      "audit_th_monitor": "Монитор",
      "audit_th_type": "Тип",
      "audit_th_violation": "Нарушение",
      "audit_th_action": "Действие",
      "audit_th_status": "Статус",
      "audit_empty_row": "Инцидентов пока нет. Система ожидает появления подозрительного контента.",
      "audit_status_active": "АКТИВЕН",
      "audit_status_resolved_safe": "СНЯТ: ПОТОК БЕЗОПАСЕН",
      "audit_status_resolved_timeout": "СНЯТ: ТАЙМИНГ ИСТЕК",
      "audit_tag_cue_lookahead": "ПРЕВЕНТИВНЫЙ (ПЛЕЕР)",
      "audit_tag_cue_timing": "ТАЙМИНГ (ПЛЕЕР)",
      "audit_tag_nsfw": "НАГОТА (NSFW)",
      "audit_tag_ocr": "СТОП-СЛОВО (OCR)",
      "audit_default_monitor": "Основной монитор",
      "audit_default_action": "Защитный экран в OBS включен, звук заглушен",

      // Modal 1: Stopwords Manager
      "modal_stopwords_title": "Словарь стоп-слов (OCR детекция)",
      "modal_stopwords_desc": "Введите запрещенные слова и фразы через новую строку. Движок отслеживает совпадения на экране и в субтитрах.",
      "modal_stopwords_examples": "Примеры безопасных маскированных правил:",
      "modal_stopwords_placeholder": "Введите запрещенные слова по одному в строке...",
      "modal_stopwords_active_rules": "Активных правил: {count}",
      "modal_stopwords_btn_close": "Закрыть",
      "modal_stopwords_btn_save": "Сохранить правила",

      // Modal 2: Hotkeys
      "modal_hotkeys_title": "Горячие клавиши (Hotkeys)",
      "modal_hotkeys_desc": "Глобальные комбинации клавиш для быстрого управления во время стрима.",
      "modal_hotkeys_th_action": "Действие",
      "modal_hotkeys_th_combo": "Комбинация",
      "modal_hotkeys_th_desc": "Описание",
      "modal_hotkeys_act1": "Экстренная цензура",
      "modal_hotkeys_act1_desc": "Мгновенно включает защитный экран и глушит аудио",
      "modal_hotkeys_act2": "Снять цензуру",
      "modal_hotkeys_act2_desc": "Принудительно скрывает экран цензуры и включает звук",
      "modal_hotkeys_act3": "Переключить OCR",
      "modal_hotkeys_act3_desc": "Включение / выключение распознавания текста на экране",
      "modal_hotkeys_act4": "Режим работы",
      "modal_hotkeys_act4_desc": "Циклическое переключение режимов защиты (Плеер / Экран / Гибрид / Выкл)",
      "modal_hotkeys_btn_close": "Закрыть",

      // Modal 3: About
      "modal_about_title": "О программе blewred",
      "modal_about_desc": "blewred — высокопроизводительный локальный комплекс превентивной защиты видеопотока стримера от нежелательного контента (NSFW) и запрещенных стоп-слов.",
      "modal_about_ver_label": "Версия:",
      "modal_about_core": "Движок: Rust + ONNX Runtime (DirectML / DirectX 12)",
      "modal_about_frontend": "Интерфейс: Tauri v2 + Vanilla JS + Bootstrap Dark",
      "modal_about_license": "Лицензия: MIT License",
      "modal_about_docs_link": "Документация и руководства",
      "modal_about_btn_close": "Закрыть",

      // Modal 4: Cues Manager
      "modal_cues_title": "Менеджер таймингов видеоплеера",
      "modal_cues_subtitle": "Запланированные интервалы цензуры для текущего видео",
      "modal_cues_video_label": "Текущее видео:",
      "modal_cues_count_badge": "Таймингов: {count}",
      "modal_cues_th_start": "Начало",
      "modal_cues_th_end": "Конец",
      "modal_cues_th_duration": "Длительность",
      "modal_cues_th_reason": "Причина",
      "modal_cues_th_actions": "Действия",
      "modal_cues_empty": "Нет запланированных таймингов для текущего видео.",
      "modal_cues_add_title": "Добавить тайминг вручную",
      "modal_cues_ph_start": "Начало (сек или М:СС)",
      "modal_cues_ph_end": "Конец (сек или М:СС)",
      "modal_cues_ph_reason": "Причина (напр. NSFW сцена)",
      "modal_cues_btn_add": "Добавить",
      "modal_cues_btn_export": "Экспорт JSON",
      "modal_cues_btn_import": "Импорт JSON",
      "modal_cues_btn_close": "Закрыть",

      // Modal 5: AI Models Download
      "modal_models_title": "Загрузка нейросетевых моделей",
      "modal_models_subtitle": "Локальные модели для классификации NSFW и оптического распознавания текста (OCR)",
      "modal_models_nsfw_title": "NSFW Детектор (ONNX)",
      "modal_models_nsfw_desc": "Компактная сверточная сеть для классификации наготы и запрещенного визуала",
      "modal_models_ocr_title": "Русский OCR (WinRT / ONNX)",
      "modal_models_ocr_desc": "Пакет распознавания кириллического текста и стоп-слов",
      "modal_models_ready": "Готово",
      "modal_models_downloading": "Загрузка: {p}%",
      "modal_models_not_installed": "Не установлено",
      "modal_models_btn_download": "Скачать",
      "modal_models_btn_close": "Закрыть",

      // Modal 6: Browser Extension Guide
      "modal_ext_title": "Расширение для браузера",
      "modal_ext_subtitle": "Синхронизация таймингов YouTube, Twitch и других плееров с движком blewred",
      "modal_ext_step1": "1. Откройте страницу расширений в Chrome или Firefox (chrome://extensions)",
      "modal_ext_step2": "2. Включите «Режим разработчика» в правом верхнем углу",
      "modal_ext_step3": "3. Нажмите «Загрузить распакованное расширение» и выберите папку browser-extension",
      "modal_ext_step4": "4. Иконка blewred появится на панели, статус станет зеленым «Подключено»",
      "modal_ext_btn_close": "Закрыть",

      // Modal 7: OBS Setup Prompt
      "modal_obs_title": "Автонастройка источника в OBS Studio",
      "modal_obs_desc": "blewred может автоматически добавить источник Censor Shield в вашу текущую сцену OBS Studio через WebSocket.",
      "modal_obs_port_label": "Порт OBS WebSocket:",
      "modal_obs_pass_label": "Пароль (если задан):",
      "modal_obs_btn_cancel": "Отмена",
      "modal_obs_btn_continue": "Продолжить автонастройку",

      // OBS Censor Shield Overlay
      "shield_violation_caption": "ЗАФИКСИРОВАННЫЙ КОНТЕНТ",
      "shield_badge_safe": "ПОТОК БЕЗОПАСЕН",
      "shield_badge_active": "ЦЕНЗУРА АКТИВНА",
      "shield_badge_timing": "ТАЙМИНГ ПЛЕЕРА",
      "shield_prob_caption": "ВЕРОЯТНОСТЬ НАРУШЕНИЯ",
      "shield_countdown_label": "ДО ОКОНЧАНИЯ",
      "shield_countdown_text": "ОСТАЛОСЬ: {sec} СЕК",
      "shield_reason_safe": "ПОТОК БЕЗОПАСЕН",
      "shield_reason_nsfw": "НЕДОПУСТИМЫЙ КОНТЕНТ (NSFW)",
      "shield_ocr_prefix": "OCR стоп-слово: {word}",
      "shield_scheduled_block": "ЗАПЛАНИРОВАННАЯ БЛОКИРОВКА",
      "shield_donate_support": "ПОДДЕРЖКА РАЗРАБОТКИ",
      "shield_donate_scan": "Ссылка для поддержки проекта: web.tribute.tg/e/1dW",

      // Lower Selectors Grid
      "label_stream_monitor": "Монитор стрима:",
      "opt_stream_obs_feed": "🎥 Поток OBS Studio (Окно / Игра / Экран)",
      "label_hud_monitor": "Монитор для HUD:",
      "opt_hud_auto": "Авто (Экран стримера)",
      "btn_test_hud": "Тест HUD",
      "label_obs_scene": "Сцена OBS:",
      "opt_scene_auto": "(Автоопределение...)",
      "btn_refresh_scenes": "Обновить",

      // Sensitivity Slider & Selective Blur
      "slider_sensitivity_title": "Чувствительность каскада ИИ (ViT + NudeNet)",
      "track_soft": "Мягкая (20%)",
      "track_strict": "Строгая (95%)",
      "selective_censor_title": "Выборочная блокировка зон в OBS (блюр источника):",
      "tuning_panel_title": "Профиль контента и точная настройка моделей",
      "btn_toggle_advanced_tuning": "Тонкая настройка",
      "lbl_content_profile": "Шаблон контента (готовые пресеты):",
      "profile_gaming": "Игры / 3D & Аниме",
      "profile_reallife": "Реальный контент / IRL",
      "profile_strict": "Макс. защита",
      "profile_custom": "Пользовательский",
      "profile_gaming_desc": "Игровой режим: защита от ложных срабатываний на 3D-полигоны, тени персонажей, стилизацию и аниме-текстуры.",
      "profile_reallife_desc": "Реальный контент: сбалансированная детекция реальной анатомии человека для вебкамер и IRL-стримов.",
      "profile_strict_desc": "Строгий режим: агрессивная мгновенная цензура при малейшем подозрении на открытые зоны.",
      "profile_custom_desc": "Пользовательский режим: активны ваши индивидуальные правила порогов и фильтрации.",
      "lbl_exact_rule_string": "Точная строка правил и порогов моделей:",
      "ph_exact_rule_string": "preset:gaming,vit_filter:on,min_conf:0.45,hold:12",
      "btn_apply_rule": "Применить",
      "btn_reset_rule": "Сброс",
      "exact_rule_syntax_hint": "Синтаксис: preset:gaming|reallife|strict, vit_filter:on|off, min_conf:0.45, hold:12, breasts:0.65, buttocks:0.60, genitalia:0.50, anus:0.60",
      "rule_status_synced": "Синхронизировано",
      "rule_status_custom": "Пользовательские",
      "tuning_vit_filter_title": "Фильтр 3D / Аниме в ViT",
      "tuning_vit_filter_desc": "Игнорировать ложный NSFW-скор от игровых рисунков и cel-shading",
      "tuning_hold_title": "Удержание трекера (кадры)",
      "tuning_hold_desc": "Для быстрых поворотов игровой камеры рекомендуется 10–15 кадров.",
      "tuning_nudenet_cutoff_title": "Минимальный порог анатомии (NudeNet)",
      "tuning_nudenet_cutoff_desc": "Порог отсечки теней полигонов и складок скинов (в играх рекомендуется 0.40 - 0.55).",
      "tuning_toast_applied": "Правила моделей обновлены",
      "btn_open_profiles_folder": "Папка профилей",
      "btn_delete_profile": "Удалить",
      "lbl_save_profile_section": "Сохранение текущей настройки в профиль:",
      "ph_new_profile_name": "Название профиля (например: Cyberpunk 2077)...",
      "btn_save_profile": "Сохранить",
      "confirm_delete_profile": "Удалить сохраненный профиль «{name}»?",
      "toast_profile_saved": "Профиль «{name}» успешно сохранен",
      "toast_profile_deleted": "Профиль удален",
      "toast_profile_cannot_delete_preset": "Встроенные пресеты Gaming, RealLife и Strict нельзя удалить",
      "toast_profile_name_empty": "Введите название профиля",

      // Video Stream Telemetry Grid
      "stat_screen_capture": "Захват экрана",
      "stat_obs_resolution": "Разрешение OBS",
      "stat_censor_shield": "Защитный экран",
      "stat_analysis_fps": "Частота анализа",
      "stat_hw_accel": "Аппаратное ускорение",
      "stat_gpu_detecting": "Определение GPU...",
      "stat_val_active": "Активен",
      "stat_val_hidden_obs": "Скрыт в OBS",
      "stat_val_waiting_obs": "Ожидание OBS",
      "stat_val_shield_on": "ON — заставка показана в OBS",
      "stat_val_shield_ready": "READY — готов к показу при нарушениях",
      "stat_val_shield_off": "OFF — выключен",

      // AI Surveillance Panel & Cascade
      "panel_ai_surveillance": "Нейросети анализа видеопотока",
      "monitor_cascade_title": "КАСКАД ИИ (ViT + 640m)",
      "gauge_frame_class": "Класс кадра:",
      "gauge_cascade_score": "Оценка каскада",
      "meter_clean": "0% Чисто",
      "meter_threshold_prefix": "Порог цензуры:",
      "stage1_title": "Этап 1: ViT",
      "stage2_title": "Этап 2: 640m",
      "status_pill_safe": "ПОТОК БЕЗОПАСЕН",
      "status_pill_active": "ЦЕНЗУРА АКТИВНА",
      "status_pill_clean": "ТЕКСТ ЧИСТ",
      "status_pill_stopword": "СТОП-СЛОВО В КАДРЕ",
      "status_pill_disabled": "ОТКЛЮЧЕН",
      "class_tag_neutral": "НЕЙТРАЛЬНО",
      "no_anatomy_detected": "Анатомии не найдено",
      "monitoring_active": "Мониторинг активен",
      "scanning_active": "Сканирование активно",
      "frame_timestamp_prefix": "Кадр:",

      // OCR Detector
      "monitor_ocr_title": "OCR ДЕТЕКТОР СТОП-СЛОВ",
      "ocr_preview_header": "Текст на экране (WinRT OCR):",
      "ocr_detected_label": "Зафиксированные стоп-слова:",
      "ocr_badge_none": "НЕТ СТОП-СЛОВ В КАДРЕ",
      "ocr_no_text_detected": "(текст на выбранном мониторе не обнаружен)",
      "ocr_not_scanned": "(Текст на экране не сканируется)",

      // FPS Scanning Mode Switch
      "fps_switch_title": "Режим сканирования видео",
      "fps_mode_idle": "Фоновый скан (5 FPS)",
      "fps_mode_boosted": "Усиленный (60 FPS)",
      "fps_mode_dynamic": "Динамический разгон",
      "fps_mode_player": "Плеер Lookahead",
      "fps_mode_standby": "Standby (Пауза)",

      // Modals Headers & Labels
      "modal_about_header": "О программе",
      "modal_about_version": "Версия:",
      "modal_about_license_label": "Лицензия:",
      "modal_about_architecture": "Архитектура:",
      "modal_about_ui_design": "UI Дизайн:",
      "modal_stopwords_header": "Словарь запретных слов и правил OCR",
      "modal_hotkeys_header": "Горячие клавиши и управление (Hotkeys)",
      "modal_hotkeys_intro": "В blewred встроены глобальные и контекстные горячие клавиши для мгновенного реагирования и управления защитой эфира:",
      "modal_models_header": "Нейросетевые модели ИИ (blewred AI Assets)",
      "modal_ext_header": "Расширение blewred для браузера (Lookahead Interceptor)",
      "modal_cues_header": "Планировщик таймингов и превентивных оповещений",
      "cues_player_waiting": "Ожидание плеера...",
      "modal_obs_header": "Автонастройка OBS Studio",

      // Additional UI Elements
      "guide_btn_aria": "Рекомендуемые шаги настройки",
      "guide_btn_close_aria": "Закрыть",
      "setup_guide_progress_badge": "Готовность: {ready} из {total}",
      "step_gpu_sub": "DirectML аппаратное ускорение (8–14 мс)",
      "step_gpu_desc": "Штатный графический драйвер GPU + DirectML. Назначьте высокую производительность для blewred.exe и включите HAGS.",
      "btn_win_gpu_settings": "Настройки графики Windows",
      "step_models_sub": "ViT скринер + NudeNet 640m локализатор",
      "step_models_desc": "Проверьте наличие и целостность SHA-256 моделей в models/ или скачайте в 1 клик.",
      "btn_models_manager": "Загрузка моделей",
      "step_ext_sub": "Синхронизация таймлайна плеера и таймингов",
      "step_ext_desc": "Включите «Режим разработчика» в chrome://extensions и загрузите распакованную папку.",
      "btn_open_ext_folder": "Открыть папку расширения",
      "step_obs_sub": "Аппаратный шейдерный фильтр & Заставка Shield",
      "step_obs_desc": "Включите WebSocket на порту 4455 и запустите автоматическую привязку плагина и заставки.",
      "btn_auto_setup_obs": "Автонастройка OBS",
      "link_detailed_docs": "Подробная инструкция со скриншотами (Docs)",
      "lookahead_monitor_title": "ПРЕВЕНТИВНЫЙ МОНИТОР СТРИМЕРА (LOOKAHEAD)",
      "lookahead_severity_obvious": "ОЧЕВИДНЫЙ NSFW",
      "lookahead_severity_suspicious": "ПОДОЗРИТЕЛЬНЫЙ КОНТЕНТ (НЕОЧЕВИДНЫЙ)",
      "lookahead_player_alloha": "Плеер Alloha",
      "lookahead_preview_img_alt": "Кадр без блюра",
      "lookahead_detected_content": "ОБНАРУЖЕННЫЙ КОНТЕНТ:",
      "lookahead_time_to_show": "ВРЕМЯ ДО ПОКАЗА ЗРИТЕЛЯМ:",
      "lookahead_btn_block_screen": "ЗАБЛОКИРОВАТЬ ЭКРАН В OBS",
      "lookahead_btn_allow_safe": "ПРОПУСТИТЬ (БЕЗОПАСНО)",
      "lookahead_btn_open_window": "Окно упреждающего видео",
      "lookahead_btn_detach_window": "ОТКРЫТЬ ОКНО (LOOKAHEAD)",
      "lookahead_btn_popout": "В отдельное окно",
      "lookahead_window_header_badge": "УПРЕЖДАЮЩИЙ ПОТОК (LOOKAHEAD)",
      "lookahead_window_safe": "БЕЗОПАСНО",
      "lookahead_window_threat": "ОБНАРУЖЕНА УГРОЗА",
      "lookahead_window_pin": "Поверх всех окон",
      "lookahead_window_pinned": "Закреплено",
      "lookahead_window_unpinned": "Не закреплено",
      "lookahead_window_test_frame": "Тестовый кадр",
      "lookahead_window_status_waiting": "Ожидание видеопотока из браузера...",
      "lookahead_window_status_sub": "Откройте видео в плеере браузера с активным расширением blewred",
      "lookahead_window_drag_hint": "Перетаскивайте за верхнюю панель в любое место экрана",
      "lookahead_window_img_alt": "Опережающий кадр видеопотока",
      "lookahead_window_full_frame_threat": "УГРОЗА (ПОЛНЫЙ КАДР)",
      "lookahead_window_box_threat": "ОБНАРУЖЕНО НАРУШЕНИЕ",
      "ext_btn_lookahead_window": "Окно упреждающего видео",
      "sensitivity_balanced_desc": "Сбалансированный режим: стандартная блокировка наготы без ложных тревог.",
      "sensitivity_soft_desc": "Мягкий режим: реагирует только на 100% явную наготу (открытые органы). Минимальная нагрузка, ноль ложных тревог.",
      "sensitivity_strict_desc": "Строгий режим: максимальная чувствительность. Размывает спорные ракурсы, частичную наготу и тени.",
      "chip_genitalia": "Гениталии",
      "chip_breasts": "Женская грудь",
      "chip_buttocks": "Ягодицы / анус",
      "chip_underwear": "Белье / бикини",
      "chip_body_exposed": "Мужской торс",
      "threshold_balanced_suffix": "Сбалансированная",
      "threshold_soft_suffix": "Мягкая",
      "threshold_strict_suffix": "Строгая",
      "live_stage1_neutral": "Нейтрально (100%)",
      "live_boxes_badge": "{count} боксов",
      "live_ocr_initializing": "Инициализация сканирования рабочего стола...",
      "mute_countdown_prefix": "Цензура:",
      "unit_seconds_short": "сек",
      "fps_stat_val_5": "5 кадр/сек",
      "fps_stat_val_60": "60 кадр/сек",
      "meter_threshold_label": "Порог: {val}%",
      "audit_header_title": "Журнал инцидентов цензуры",
      "audit_btn_clear_text": "Очистить",
      "audit_stat_latency_title": "Задержка",
      "audit_stat_health_title": "Защита",
      "audit_tab_all": "Все",
      "audit_tab_nsfw": "Нагота",
      "audit_tab_ocr": "Стоп-слова",
      "audit_tab_cue": "Тайминги",
      "audit_tab_active": "Активные",
      "ru_ocr_missing_title": "Внимание:",
      "ru_ocr_missing_desc1": "Языковой модуль WinRT OCR для русского языка (ru-RU) не установлен в Windows.",
      "ru_ocr_missing_desc2": "Без него распознавание русских стоп-слов ограничено.",
      "btn_install_ru_ocr": "Установить модуль RU OCR",
      "en_ocr_missing_title": "Внимание:",
      "en_ocr_missing_desc1": "Языковой модуль WinRT OCR для английского языка (en-US) не установлен в Windows.",
      "en_ocr_missing_desc2": "Без него распознавание английских стоп-слов ограничено.",
      "btn_install_en_ocr": "Установить модуль EN OCR",
      "btn_open_ocr_settings": "Настройки языков Windows",
      "stopwords_format_info": "Построчный ввод. Поддерживаются точные совпадения, подстановочные маски слово*, регулярные выражения /шаблон/ и омоглифы кириллицы/латиницы.",
      "stopwords_rules_applied_live": "Правила применяются на лету при сохранении",
      "stopwords_rules_count": "{count} правил",
      "btn_save_rules": "Применить изменения",
      "hotkeys_th_key": "Клавиша",
      "hotkeys_th_action": "Действие",
      "hotkeys_th_desc": "Описание",
      "hotkeys_row_f9_title": "Panic Mute / Shield",
      "hotkeys_row_f9_desc": "Экстренное переключение (toggle) Censor Shield и заглушения в OBS: скрывает открытую заставку или включает защиту. В окне Lookahead — подтверждает блокировку сцены.",
      "hotkeys_row_f8_title": "Threat Boost / Skip",
      "hotkeys_row_f8_desc": "Переключение мониторинга между фоновым (5 FPS) и форсированным (60 FPS). В окне Lookahead — пропуск предупреждения (ложная тревога).",
      "hotkeys_row_ctrlv_title": "Парсинг таймингов",
      "hotkeys_row_ctrlv_desc": "Вставка скриншота расписания из буфера обмена для автоматического распознавания через WinRT OCR в список планировщика.",
      "hotkeys_row_modes_title": "Режимы работы",
      "hotkeys_row_modes_desc": "Быстрое переключение: 0 — Гибридный, 1 — Только плеер, 2 — Только экран, 3 — Защита выключена (Standby).",
      "hotkeys_row_mode_cycle_title": "Смена режима защиты",
      "hotkeys_row_mode_cycle_desc": "Циклическое переключение режимов защиты эфира: 0 (Гибридный) ➔ 1 (Только плеер) ➔ 2 (Только экран) ➔ 3 (Standby / Выкл). При смене выводится HUD-уведомление на выбранном мониторе.",
      "audit_header_title": "Журнал инцидентов цензуры",
      "audit_btn_clear_text": "Очистить",
      "audit_btn_detach": "В отдельное окно",
      "audit_window_badge": "ЖУРНАЛ ИНЦИДЕНТОВ (AUDIT LOG)",
      "audit_window_title": "Журнал инцидентов и аудита",
      "audit_stat_total": "Всего",
      "audit_stat_nsfw": "NSFW",
      "audit_stat_ocr": "Стоп-слова",
      "audit_stat_cues": "Тайминги",
      "audit_tab_all": "Все",
      "audit_tab_nsfw": "Нагота",
      "audit_tab_ocr": "Стоп-слова",
      "audit_tab_cue": "Тайминги",
      "audit_tab_active": "Активные",
      "audit_th_time": "Время",
      "audit_th_monitor": "Монитор",
      "audit_th_type": "Тип",
      "audit_th_violation": "Нарушение",
      "audit_th_action": "Действие в OBS",
      "audit_th_status": "Статус",
      "audit_empty_row": "Инцидентов цензуры не зафиксировано. Поток безопасен.",
      "mode_hud_title": "СМЕНА РЕЖИМА",
      "mode_name_0": "ГИБРИДНЫЙ РЕЖИМ (HYBRID)",
      "mode_name_1": "ТОЛЬКО ПЛЕЕР (PLAYER ONLY)",
      "mode_name_2": "ТОЛЬКО ЭКРАН (SCREEN ONLY)",
      "mode_name_3": "ЗАЩИТА ВЫКЛЮЧЕНА (STANDBY)",
      "mode_desc_0": "Защита экрана и видеоплеера активна",
      "mode_desc_1": "Захват экрана отключен, CPU < 1%",
      "mode_desc_2": "Непрерывный мониторинг рабочего стола",
      "mode_desc_3": "Мониторинг приостановлен, цензура выключена",
      "btn_pin_window": "Поверх всех окон",
      "btn_unpin_window": "Обычный режим",
      "btn_minimize_window": "Свернуть",
      "btn_close_window": "Закрыть",
      "hotkeys_scope_label": "Область действия горячих клавиш:",
      "hotkeys_scope_global": "Глобально в Windows",
      "hotkeys_scope_global_desc": "Работают в играх, браузере и любых других окнах",
      "hotkeys_scope_local": "Локально в приложении",
      "hotkeys_scope_local_desc": "Срабатывают только когда активно окно blewred",
      "hotkeys_rebind_title": "Назначение пользовательских клавиш",
      "hotkeys_rebind_desc": "Нажмите на кнопку с клавишей и нажмите нужную комбинацию (например, Ctrl+Shift+F9):",
      "hotkeys_press_key": "Нажмите комбинацию клавиш...",
      "hotkeys_btn_reset": "По умолчанию",
      "hotkeys_btn_save": "Сохранить привязки",
      "hotkeys_btn_record": "Записать",
      "hotkeys_btn_recording": "Запись...",
      "hotkeys_recording_prompt": "Нажмите комбинацию (напр. Ctrl+Shift+F9)...",
      "hotkeys_release_hint": "отпустите любую клавишу",
      "hotkeys_holding_prefix": "Удерживается: ",
      "hotkeys_opt_fn": "Клавиши F1–F24",
      "hotkeys_opt_digits": "Цифры",
      "hotkeys_opt_letters": "Буквы",
      "hotkeys_opt_special": "Специальные",
      "models_desc_directml": "Для работы превентивной защиты blewred использует два каскада нейросетей DirectML: классификатор кадров vit_nsfw.onnx и детектор анатомических зон 640m.onnx.",
      "models_card1_title": "1. Классификатор кадров (ViT NSFW)",
      "models_card2_title": "2. Детектор анатомии (NudeNet 640m)",
      "models_speed_label": "Скорость загрузки:",
      "models_eta_label": "Примерное время до завершения:",
      "models_btn_start_download": "Загрузить модели",
      "ext_desc_lead": "Расширение перехватывает видеопоток онлайн-плееров (Kinobox, Alloha, Collaps, Rutube и др.) до показа зрителям стрима, обеспечивая нулевую задержку цензуры и CPU < 1%.",
      "ext_btn_launch_browser": "Запустить браузер с расширением",
      "ext_btn_open_folder": "Открыть папку расширения",
      "ext_guide_panel_title": "Инструкция по установке в Chromium (Chrome, Edge, Brave, Opera, Yandex):",
      "ext_step1_text": "Откройте страницу расширений в браузере: chrome://extensions или edge://extensions.",
      "ext_step2_text": "Включите тумблер «Режим разработчика» (Developer mode) в верхнем правом углу.",
      "ext_step3_text": "Нажмите кнопку «Загрузить распакованное расширение» (Load unpacked).",
      "ext_step4_text": "В открывшемся окне выберите папку extensions/chrome из каталога приложения (кнопка выше откроет её).",
      "ext_step5_text": "Значок blewred появится на панели расширений. При просмотре видео расширение автоматически активирует превентивный буфер.",
      "cues_autocensor_label": "Автоматическая цензура:",
      "cues_autocensor_status_on": "Включена (OBS + звук)",
      "cues_autocensor_status_off": "Отключена (Только HUD)",
      "cues_notifications_label": "Всплывающие уведомления (HUD):",
      "cues_notifications_status_on": "Включены (HUD)",
      "cues_notifications_status_off": "Отключены",
      "cues_prewarn_label": "Упреждение:",
      "unit_seconds": "сек",
      "cues_mode_label": "Режим списка:",
      "cues_mode_append": "Добавлять",
      "cues_mode_replace": "Заменять",
      "cues_paste_panel_heading": "Вставка скриншота или текста с таймингами",
      "cues_ocr_status_badge": "Windows Media OCR (Нативный / Готово)",
      "cues_dropzone_title": "Нажмите сюда и нажмите Ctrl+V для вставки скриншота (или перетащите файл)",
      "cues_dropzone_sub": "Умный парсер понимает любые разделители и форматы: 34:69-1.34 (нормализует в 35:09 – 01:34:00), 12.30 - 14.15 (сцена), двоеточия, точки, дефисы и тире.",
      "cues_preview_title": "Скриншот загружен",
      "cues_preview_meta": "Готов к распознаванию через Windows Media OCR",
      "btn_cue_reset": "Сбросить",
      "btn_cue_recognize_ocr": "Распознать через Windows OCR",
      "cues_manual_text_label": "Или вставьте/отредактируйте текст таймингов вручную:",
      "btn_cue_parse_manual": "Парсить этот текст",
      "cues_manual_placeholder": "Например: 12:45-14:20 Непристойный диалог\n34:69-1.34 Опасная сцена",
      "cues_list_heading": "Список запланированных таймингов",
      "btn_clear_all_cues": "Очистить все",
      "cues_th_status": "Статус",
      "cues_th_interval": "Интервал",
      "cues_th_duration": "Длительность",
      "cues_th_reason": "Описание / Причина",
      "cues_th_action": "Действие",
      "cues_empty_table_text": "Нет запланированных таймингов. Вставьте скриншот с таймкодами через Ctrl+V.",
      "cues_footer_sync": "Синхронизация с расширением активна (порт 51789)",
      "obs_setup_status_label": "Статус OBS Studio:",
      "obs_status_not_running": "OBS не открыта",
      "obs_prompt_desc1": "Программа OBS Studio сейчас не запущена или соединение ещё не установлено.",
      "obs_prompt_desc2": "Для автоматической инъекции плагина, создания сцен Censor Shield и настройки шейдеров откройте OBS Studio, после чего нажмите кнопку «Продолжить автонастройку».",
      "btn_launch_obs_text": "Запустить OBS Studio",
      "obs_launch_manual_hint": "или откройте OBS вручную",
      "obs_setup_in_progress": "Выполняется автонастройка...",
      "btn_cancel_text": "Отмена",
      "btn_continue_setup_text": "Продолжить автонастройку",
      "shield_brand_alt": "blewred",
      "shield_badge_censor_active": "CENSOR ACTIVE",
      "shield_caption_detected": "ЗАФИКСИРОВАННЫЙ КОНТЕНТ",
      "shield_target_nsfw": "НЕДОПУСТИМЫЙ КОНТЕНТ (NSFW)",
      "shield_prob_caption_text": "ВЕРОЯТНОСТЬ",
      "checking": "Проверка...",
      "disconnected": "Не подключено",
      "btn_close": "Закрыть",
      "nav_tray_aria": "Свернуть в трей",
      "modal_close_title": "Закрытие blewred",
      "modal_close_lead": "Вы собираетесь закрыть окно приложения. Выберите желаемое действие:",
      "modal_close_btn_tray": "Свернуть в трей",
      "modal_close_recommended": "Рекомендуется",
      "modal_close_tray_desc": "blewred продолжит работу в фоне: превентивный анализ, детекция стоп-слов и защита OBS останутся активными. Вы сможете развернуть окно в любой момент из системного трея.",
      "modal_close_btn_exit": "Закрыть окончательно",
      "modal_close_exit_desc": "Полная остановка всех служб защиты, выгрузка нейросетей каскада ИИ и чистое завершение процесса blewred.",
      "modal_close_remember": "Запомнить мой выбор и больше не спрашивать",
      "modal_close_btn_cancel": "Отмена",
      "setting_close_action_label": "При нажатии на крестик:",
      "setting_close_action_ask": "Спрашивать каждый раз",
      "setting_close_action_tray": "Сворачивать в трей",
      "setting_close_action_exit": "Закрывать окончательно",
      "modal_disable_support_title": "Отключение показа поддержки",
      "modal_disable_support_p1": "Это приложение является бесплатным open-source проектом без рекламы и имеет опцию отключения показа поддержки на заставке.",
      "modal_disable_support_p2": "Разработка расходует время и ресурсы, а поддержка, в свою очередь, помогает понять, что они были потрачены не зря.",
      "modal_disable_support_p3": "По вашему желанию, Вы можете убрать блок с поддержкой, чтобы зрители стрима его не видели",
      "modal_disable_support_btn_yes": "Да",
      "modal_disable_support_btn_no": "Нет"
    },

    en: {
      // Header Navigation & Status Ribbon
      "nav_obs": "OBS Studio",
      "nav_obs_plugin": "OBS Plugin",
      "nav_screen": "Video Capture",
      "nav_shield": "OBS Shield",
      "nav_extension": "Extension",
      "nav_vision": "Vision Mode",
      "status_off": "OFF",
      "status_live": "LIVE",
      "status_active": "Active",
      "status_waiting": "Waiting...",
      "status_paused": "Paused",
      "status_scan_idle": "Idle Scan",
      "status_scan_dynamic": "Dynamic 60 FPS",

      // Streamer Setup Guide
      "setup_guide_title": "Streamer Quick Start: Recommended Setup Steps",
      "setup_guide_progress": "Ready: {ready} of {total}",
      "setup_step1_title": "GPU Hardware Acceleration (DirectML)",
      "setup_step1_desc": "Utilizing DirectML (DirectX 12) for instantaneous NSFW and OCR neural inference.",
      "setup_step1_cuda_active": "DirectML Active",
      "setup_step1_cpu_mode": "CPU Mode",
      "setup_step2_title": "Local Neural Network Models",
      "setup_step2_desc": "Local model weights for NSFW detection and stopword OCR recognition.",
      "setup_step2_btn": "Download Models",
      "setup_step3_title": "Browser Extension",
      "setup_step3_desc": "Transmits precise player timings and video metadata directly to the engine.",
      "setup_step3_btn": "Installation Guide",
      "setup_step4_title": "OBS Studio Source Setup",
      "setup_step4_desc": "Adds 'Browser' source with Censor Shield for automatic censorship.",
      "setup_step4_btn": "OBS Auto-Setup",
      "setup_dont_show": "Do not show this checklist again on startup",

      // Toolbar Controls
      "toolbar_cues_mgr": "Player Timings",
      "toolbar_stopwords": "Stopwords (OCR)",
      "toolbar_nsfw_test": "Test NSFW",
      "toolbar_ocr_label": "OCR Detection:",
      "toolbar_hotkeys": "Hotkeys",

      // Stream & Censor Control Panel
      "stream_control_title": "Stream & Censor Control",
      "stream_guard_active": "PROTECTION ACTIVE",
      "stream_guard_disabled": "PROTECTION DISABLED",
      "mode_player_title": "Preemptive (Player)",
      "mode_player_desc": "Inspects only video player window via lookahead extension timings",
      "mode_screen_title": "Full Screen",
      "mode_screen_desc": "Continuous monitoring of selected display (games, desktop, chat)",
      "mode_hybrid_title": "Hybrid",
      "mode_hybrid_desc": "Smart focus: player priority during playback, screen watch on pause",
      "mode_off_title": "Protection Off",
      "mode_off_desc": "Detection and censorship disabled. Video stream passes without analysis.",
      "stream_monitor_label": "Capture Monitor:",
      "stream_shield_label": "Censor Shield Overlay:",
      "stream_shield_desc": "Shows censor shield overlay in OBS upon violation",
      "stream_donate_label": "Development Support Banner:",
      "stream_donate_desc": "Link to support development",
      "stream_fps_label": "Scanning Frequency (Idle):",
      "stream_fps_5": "5 FPS (Power Saver)",
      "stream_fps_60": "60 FPS (High Smoothness)",

      // Telemetry Cards
      "telemetry_title": "System & Neural Telemetry",
      "telemetry_hw_accel": "Hardware Acceleration",
      "telemetry_gpu_card": "GPU & DirectML Inference",
      "telemetry_scan_speed": "Scan Speed",
      "telemetry_latency": "Engine Latency",
      "telemetry_active_rules": "Stopword Rules",
      "telemetry_rules_suffix": "active rules",
      "telemetry_buffer": "Delay Buffer",
      "telemetry_audio_mute": "Audio Mute",
      "telemetry_audio_normal": "NORMAL",
      "telemetry_audio_muted": "MUTED",

      // Lookahead Warning HUD Panel
      "lookahead_title": "Preemptive Monitoring (Player)",
      "lookahead_badge_detected": "HAZARDOUS MOMENT DETECTED",
      "lookahead_threat_in": "Threat detected in {sec} sec",
      "lookahead_btn_block": "Block Now",
      "lookahead_btn_allow": "Allow (Safe)",
      "lookahead_unblurred_tag": "Uncensored Preview",
      "lookahead_cue_timing_label": "Scheduled Timing",

      // Incident & Audit Log Panel
      "audit_title": "Incident & Audit Log",
      "audit_stat_total": "Total:",
      "audit_stat_active": "Active:",
      "audit_stat_nsfw": "NSFW:",
      "audit_stat_ocr": "OCR:",
      "audit_stat_cues": "Timings:",
      "audit_filter_all": "All",
      "audit_filter_nsfw": "NSFW",
      "audit_filter_ocr": "Stopwords",
      "audit_filter_cues": "Timings",
      "audit_th_time": "Time",
      "audit_th_monitor": "Monitor",
      "audit_th_type": "Type",
      "audit_th_violation": "Violation",
      "audit_th_action": "Action",
      "audit_th_status": "Status",
      "audit_empty_row": "No incidents yet. System is awaiting suspicious content.",
      "audit_status_active": "ACTIVE",
      "audit_status_resolved_safe": "RESOLVED: STREAM SAFE",
      "audit_status_resolved_timeout": "RESOLVED: TIMING EXPIRED",
      "audit_tag_cue_lookahead": "PREEMPTIVE (PLAYER)",
      "audit_tag_cue_timing": "TIMING (PLAYER)",
      "audit_tag_nsfw": "NUDITY (NSFW)",
      "audit_tag_ocr": "STOPWORD (OCR)",
      "audit_default_monitor": "Primary Monitor",
      "audit_default_action": "Censor shield active in OBS, audio muted",

      // Modal 1: Stopwords Manager
      "modal_stopwords_title": "Stopwords Dictionary (OCR Detection)",
      "modal_stopwords_desc": "Enter prohibited words and phrases, one per line. The engine monitors matches on screen and in subtitles.",
      "modal_stopwords_examples": "Examples of safe masked rules:",
      "modal_stopwords_placeholder": "Enter prohibited words one per line...",
      "modal_stopwords_active_rules": "Active rules: {count}",
      "modal_stopwords_btn_close": "Close",
      "modal_stopwords_btn_save": "Save Rules",

      // Modal 2: Hotkeys
      "modal_hotkeys_title": "Keyboard Shortcuts (Hotkeys)",
      "modal_hotkeys_desc": "Global key combinations for quick control during live streaming.",
      "modal_hotkeys_th_action": "Action",
      "modal_hotkeys_th_combo": "Combination",
      "modal_hotkeys_th_desc": "Description",
      "modal_hotkeys_act1": "Emergency Censor",
      "modal_hotkeys_act1_desc": "Instantly activates censor shield and mutes audio",
      "modal_hotkeys_act2": "Clear Censor",
      "modal_hotkeys_act2_desc": "Forcefully hides censor shield and unmutes audio",
      "modal_hotkeys_act3": "Toggle OCR",
      "modal_hotkeys_act3_desc": "Enable / disable on-screen text recognition",
      "modal_hotkeys_act4": "Operation Mode",
      "modal_hotkeys_act4_desc": "Cycle protection modes (Player / Full Screen / Hybrid / Off)",
      "modal_hotkeys_btn_close": "Close",

      // Modal 3: About
      "modal_about_title": "About blewred",
      "modal_about_desc": "blewred is a high-performance local preemptive protection suite shielding live streams from unwanted content (NSFW) and prohibited stopwords.",
      "modal_about_ver_label": "Version:",
      "modal_about_core": "Engine: Rust + ONNX Runtime (DirectML / DirectX 12)",
      "modal_about_frontend": "Frontend: Tauri v2 + Vanilla JS + Bootstrap Dark",
      "modal_about_license": "License: MIT License",
      "modal_about_docs_link": "Documentation & Guides",
      "modal_about_btn_close": "Close",

      // Modal 4: Cues Manager
      "modal_cues_title": "Player Timings & Cues Manager",
      "modal_cues_subtitle": "Scheduled censor intervals for current video",
      "modal_cues_video_label": "Current Video:",
      "modal_cues_count_badge": "Cues: {count}",
      "modal_cues_th_start": "Start",
      "modal_cues_th_end": "End",
      "modal_cues_th_duration": "Duration",
      "modal_cues_th_reason": "Reason",
      "modal_cues_th_actions": "Actions",
      "modal_cues_empty": "No scheduled timings for the current video.",
      "modal_cues_add_title": "Add Cue Manually",
      "modal_cues_ph_start": "Start (sec or M:SS)",
      "modal_cues_ph_end": "End (sec or M:SS)",
      "modal_cues_ph_reason": "Reason (e.g. NSFW scene)",
      "modal_cues_btn_add": "Add",
      "modal_cues_btn_export": "Export JSON",
      "modal_cues_btn_import": "Import JSON",
      "modal_cues_btn_close": "Close",

      // Modal 5: AI Models Download
      "modal_models_title": "Neural Network Models Download",
      "modal_models_subtitle": "Local models for NSFW classification and optical character recognition (OCR)",
      "modal_models_nsfw_title": "NSFW Detector (ONNX)",
      "modal_models_nsfw_desc": "Compact convolutional network for nudity and illicit visual classification",
      "modal_models_ocr_title": "Russian OCR (WinRT / ONNX)",
      "modal_models_ocr_desc": "Cyrillic text and stopword recognition package",
      "modal_models_ready": "Ready",
      "modal_models_downloading": "Downloading: {p}%",
      "modal_models_not_installed": "Not installed",
      "modal_models_btn_download": "Download",
      "modal_models_btn_close": "Close",

      // Modal 6: Browser Extension Guide
      "modal_ext_title": "Browser Extension Guide",
      "modal_ext_subtitle": "Syncing YouTube, Twitch, and other player timings with blewred engine",
      "modal_ext_step1": "1. Open the extensions page in Chrome or Firefox (chrome://extensions)",
      "modal_ext_step2": "2. Enable 'Developer mode' in the top right corner",
      "modal_ext_step3": "3. Click 'Load unpacked' and select the browser-extension folder",
      "modal_ext_step4": "4. The blewred icon will appear in the toolbar and show green 'Connected' status",
      "modal_ext_btn_close": "Close",

      // Modal 7: OBS Setup Prompt
      "modal_obs_title": "OBS Studio Source Auto-Setup",
      "modal_obs_desc": "blewred can automatically add the Censor Shield source into your active OBS Studio scene via WebSocket.",
      "modal_obs_port_label": "OBS WebSocket Port:",
      "modal_obs_pass_label": "Password (if set):",
      "modal_obs_btn_cancel": "Cancel",
      "modal_obs_btn_continue": "Continue Auto-Setup",

      // OBS Censor Shield Overlay
      "shield_violation_caption": "DETECTED VIOLATION",
      "shield_badge_safe": "STREAM SAFE",
      "shield_badge_active": "CENSOR ACTIVE",
      "shield_badge_timing": "PLAYER CUE TIMING",
      "shield_prob_caption": "VIOLATION PROBABILITY",
      "shield_countdown_label": "COUNTDOWN",
      "shield_countdown_text": "AUTO-RESUME IN {sec} SEC",
      "shield_reason_safe": "STREAM SAFE",
      "shield_reason_nsfw": "UNACCEPTABLE CONTENT (NSFW)",
      "shield_ocr_prefix": "OCR stopword: {word}",
      "shield_scheduled_block": "SCHEDULED CENSOR BLOCK",
      "shield_donate_support": "SUPPORT DEVELOPMENT",
      "shield_donate_scan": "Link to support development: web.tribute.tg/e/1dW",

      // Lower Selectors Grid
      "label_stream_monitor": "Stream Monitor:",
      "opt_stream_obs_feed": "🎥 OBS Studio Feed (Window / Game / Display)",
      "label_hud_monitor": "HUD Monitor:",
      "opt_hud_auto": "Auto (Streamer Screen)",
      "btn_test_hud": "Test HUD",
      "label_obs_scene": "OBS Scene:",
      "opt_scene_auto": "(Auto-detect...)",
      "btn_refresh_scenes": "Refresh",

      // Sensitivity Slider & Selective Blur
      "slider_sensitivity_title": "AI Cascade Sensitivity (ViT + NudeNet)",
      "track_soft": "Soft (20%)",
      "track_strict": "Strict (95%)",
      "selective_censor_title": "Selective Zone Censorship in OBS (Source Blur):",
      "tuning_panel_title": "Content Profile & Model Fine-Tuning",
      "btn_toggle_advanced_tuning": "Fine-Tuning",
      "lbl_content_profile": "Content Preset (Ready-made templates):",
      "profile_gaming": "Gaming / 3D & Anime",
      "profile_reallife": "Real Life / IRL",
      "profile_strict": "Strict / Paranoia",
      "profile_custom": "Custom Tuning",
      "profile_gaming_desc": "Gaming mode: prevents false triggers on 3D character polygons, lighting shadows, cel-shading, and anime textures.",
      "profile_reallife_desc": "Real-life mode: balanced detection of human anatomy for webcams and IRL streaming.",
      "profile_strict_desc": "Strict mode: aggressive immediate censorship on the slightest suspicion of exposed anatomy.",
      "profile_custom_desc": "Custom mode: your fine-grained threshold and filtering rules are active.",
      "lbl_exact_rule_string": "Exact Model Rules & Cutoffs String:",
      "ph_exact_rule_string": "preset:gaming,vit_filter:on,min_conf:0.45,hold:12",
      "btn_apply_rule": "Apply",
      "btn_reset_rule": "Reset",
      "exact_rule_syntax_hint": "Syntax: preset:gaming|reallife|strict, vit_filter:on|off, min_conf:0.45, hold:12, breasts:0.65, buttocks:0.60, genitalia:0.50, anus:0.60",
      "rule_status_synced": "Synchronized",
      "rule_status_custom": "Custom Modified",
      "tuning_vit_filter_title": "ViT 3D / Anime False-Positive Filter",
      "tuning_vit_filter_desc": "Suppress false NSFW scores from game drawings and cel-shading",
      "tuning_hold_title": "Tracker Hold Retention (Frames)",
      "tuning_hold_desc": "For fast camera rotations in games, 10–15 frames is recommended.",
      "tuning_nudenet_cutoff_title": "NudeNet Minimum Anatomical Threshold",
      "tuning_nudenet_cutoff_desc": "Cutoff floor for polygon shadows and skin creases (0.40 - 0.55 recommended for games).",
      "tuning_toast_applied": "Model rules updated",
      "btn_open_profiles_folder": "Profiles Folder",
      "btn_delete_profile": "Delete",
      "lbl_save_profile_section": "Save Current Tuning as Profile:",
      "ph_new_profile_name": "Profile name (e.g. Cyberpunk 2077)...",
      "btn_save_profile": "Save",
      "confirm_delete_profile": "Delete saved profile '{name}'?",
      "toast_profile_saved": "Profile '{name}' saved successfully",
      "toast_profile_deleted": "Profile deleted",
      "toast_profile_cannot_delete_preset": "Built-in presets Gaming, RealLife, and Strict cannot be deleted",
      "toast_profile_name_empty": "Enter a profile name",

      // Video Stream Telemetry Grid
      "stat_screen_capture": "Screen Capture",
      "stat_obs_resolution": "OBS Resolution",
      "stat_censor_shield": "Censor Shield",
      "stat_analysis_fps": "Analysis Frequency",
      "stat_hw_accel": "Hardware Acceleration",
      "stat_gpu_detecting": "Detecting GPU...",
      "stat_val_active": "Active",
      "stat_val_hidden_obs": "Hidden in OBS",
      "stat_val_waiting_obs": "Waiting for OBS",
      "stat_val_shield_on": "ON — overlay active in OBS",
      "stat_val_shield_ready": "READY — primed on violation",
      "stat_val_shield_off": "OFF — disabled",

      // AI Surveillance Panel & Cascade
      "panel_ai_surveillance": "Video Stream Neural Analysis",
      "monitor_cascade_title": "AI CASCADE (ViT + 640m)",
      "gauge_frame_class": "Frame Class:",
      "gauge_cascade_score": "Cascade Score",
      "meter_clean": "0% Clean",
      "meter_threshold_prefix": "Censor Threshold:",
      "stage1_title": "Stage 1: ViT",
      "stage2_title": "Stage 2: 640m",
      "status_pill_safe": "STREAM SAFE",
      "status_pill_active": "CENSOR ACTIVE",
      "status_pill_clean": "TEXT CLEAN",
      "status_pill_stopword": "STOPWORD IN FRAME",
      "status_pill_disabled": "DISABLED",
      "class_tag_neutral": "NEUTRAL",
      "no_anatomy_detected": "No anatomy detected",
      "monitoring_active": "Monitoring active",
      "scanning_active": "Scanning active",
      "frame_timestamp_prefix": "Frame:",

      // OCR Detector
      "monitor_ocr_title": "OCR STOPWORD DETECTOR",
      "ocr_preview_header": "On-Screen Text (WinRT OCR):",
      "ocr_detected_label": "Detected stopwords:",
      "ocr_badge_none": "NO STOPWORDS IN FRAME",
      "ocr_no_text_detected": "(no text detected on selected monitor)",
      "ocr_not_scanned": "(Screen text not scanned)",

      // FPS Scanning Mode Switch
      "fps_switch_title": "Video Scanning Mode",
      "fps_mode_idle": "Idle Scan (5 FPS)",
      "fps_mode_boosted": "Boosted (60 FPS)",
      "fps_mode_dynamic": "Dynamic Boost",
      "fps_mode_player": "Player Lookahead",
      "fps_mode_standby": "Standby (Paused)",

      // Modals Headers & Labels
      "modal_about_header": "About blewred",
      "modal_about_version": "Version:",
      "modal_about_license_label": "License:",
      "modal_about_architecture": "Architecture:",
      "modal_about_ui_design": "UI Design:",
      "modal_stopwords_header": "Stopwords Dictionary (OCR Detection)",
      "modal_hotkeys_header": "Keyboard Shortcuts (Hotkeys)",
      "modal_hotkeys_intro": "blewred features built-in global and context hotkeys for instant reaction and broadcast stream protection:",
      "modal_models_header": "AI Neural Models (blewred AI Assets)",
      "modal_ext_header": "blewred Browser Extension (Lookahead Interceptor)",
      "modal_cues_header": "Timings & Preemptive Alerts Scheduler",
      "cues_player_waiting": "Waiting for player...",
      "modal_obs_header": "OBS Studio Auto-Setup",

      // Additional UI Elements
      "guide_btn_aria": "Recommended Setup Steps",
      "guide_btn_close_aria": "Close",
      "setup_guide_progress_badge": "Ready: {ready} of {total}",
      "step_gpu_sub": "DirectML hardware acceleration (8–14 ms)",
      "step_gpu_desc": "Stock GPU driver + DirectML. Assign High Performance for blewred.exe and enable HAGS.",
      "btn_win_gpu_settings": "Windows Graphics Settings",
      "step_models_sub": "ViT screener + NudeNet 640m localizer",
      "step_models_desc": "Verify presence and SHA-256 integrity of models in models/ or download in 1 click.",
      "btn_models_manager": "Download Models",
      "step_ext_sub": "Player timeline sync and timings",
      "step_ext_desc": "Enable Developer mode in chrome://extensions and load the unpacked folder.",
      "btn_open_ext_folder": "Open Extension Folder",
      "step_obs_sub": "Hardware shader filter & Shield overlay",
      "step_obs_desc": "Enable WebSocket on port 4455 and run auto-binding of plugin and shield.",
      "btn_auto_setup_obs": "Auto-Setup OBS",
      "link_detailed_docs": "Detailed instructions with screenshots (Docs)",
      "lookahead_monitor_title": "STREAMER LOOKAHEAD MONITOR",
      "lookahead_severity_obvious": "OBVIOUS NSFW",
      "lookahead_severity_suspicious": "SUSPICIOUS CONTENT (SUBTLE)",
      "lookahead_player_alloha": "Alloha Player",
      "lookahead_preview_img_alt": "Uncensored frame",
      "lookahead_detected_content": "DETECTED CONTENT:",
      "lookahead_time_to_show": "TIME BEFORE AUDIENCE SEES:",
      "lookahead_btn_block_screen": "BLOCK SCREEN IN OBS",
      "lookahead_btn_allow_safe": "ALLOW (SAFE)",
      "lookahead_btn_open_window": "Lookahead Video Window",
      "lookahead_btn_detach_window": "OPEN LOOKAHEAD WINDOW",
      "lookahead_btn_popout": "Pop out window",
      "lookahead_window_header_badge": "LOOKAHEAD STREAM",
      "lookahead_window_safe": "SAFE",
      "lookahead_window_threat": "THREAT DETECTED",
      "lookahead_window_pin": "Always on top",
      "lookahead_window_pinned": "Pinned",
      "lookahead_window_unpinned": "Unpinned",
      "lookahead_window_test_frame": "Test Frame",
      "lookahead_window_status_waiting": "Waiting for browser video stream...",
      "lookahead_window_status_sub": "Play video in browser player with active blewred extension",
      "lookahead_window_drag_hint": "Drag top bar anywhere on your desktop",
      "lookahead_window_img_alt": "Lookahead video stream frame",
      "lookahead_window_full_frame_threat": "THREAT (FULL FRAME)",
      "lookahead_window_box_threat": "VIOLATION DETECTED",
      "ext_btn_lookahead_window": "Lookahead Video Window",
      "sensitivity_balanced_desc": "Balanced mode: standard nudity censorship without false alarms.",
      "sensitivity_soft_desc": "Soft mode: triggers only on 100% explicit nudity. Minimal load, zero false alarms.",
      "sensitivity_strict_desc": "Strict mode: maximum sensitivity. Blurs questionable angles, partial nudity and shadows.",
      "chip_genitalia": "Genitalia",
      "chip_breasts": "Female Breasts",
      "chip_buttocks": "Buttocks / Anus",
      "chip_underwear": "Underwear / Bikini",
      "chip_body_exposed": "Male Torso",
      "threshold_balanced_suffix": "Balanced",
      "threshold_soft_suffix": "Soft",
      "threshold_strict_suffix": "Strict",
      "live_stage1_neutral": "Neutral (100%)",
      "live_boxes_badge": "{count} boxes",
      "live_ocr_initializing": "Initializing desktop screen scan...",
      "mute_countdown_prefix": "Censor:",
      "unit_seconds_short": "s",
      "fps_stat_val_5": "5 fps",
      "fps_stat_val_60": "60 fps",
      "meter_threshold_label": "Threshold: {val}%",
      "audit_header_title": "Censorship Incident Log",
      "audit_btn_clear_text": "Clear",
      "audit_stat_latency_title": "Latency",
      "audit_stat_health_title": "Protection",
      "audit_tab_all": "All",
      "audit_tab_nsfw": "Nudity",
      "audit_tab_ocr": "Stopwords",
      "audit_tab_cue": "Timings",
      "audit_tab_active": "Active",
      "ru_ocr_missing_title": "Warning:",
      "ru_ocr_missing_desc1": "The Windows WinRT OCR language pack for Russian (ru-RU) is not installed.",
      "ru_ocr_missing_desc2": "Without it, Russian stopword recognition is limited.",
      "btn_install_ru_ocr": "Install RU OCR Pack",
      "en_ocr_missing_title": "Warning:",
      "en_ocr_missing_desc1": "The Windows WinRT OCR language pack for English (en-US) is not installed.",
      "en_ocr_missing_desc2": "Without it, English stopword recognition is limited.",
      "btn_install_en_ocr": "Install EN OCR Pack",
      "btn_open_ocr_settings": "Windows Language Settings",
      "stopwords_format_info": "One rule per line. Exact matches, wildcard masks word*, regular expressions /pattern/, and homoglyphs are supported.",
      "stopwords_rules_applied_live": "Rules are applied on-the-fly upon saving",
      "stopwords_rules_count": "{count} rules",
      "btn_save_rules": "Apply Changes",
      "hotkeys_th_key": "Key",
      "hotkeys_th_action": "Action",
      "hotkeys_th_desc": "Description",
      "hotkeys_row_f9_title": "Panic Mute / Shield",
      "hotkeys_row_f9_desc": "Emergency toggle for Censor Shield & OBS mute: dismisses active shield or engages protection. In Lookahead window — confirms censor block.",
      "hotkeys_row_f8_title": "Threat Boost / Skip",
      "hotkeys_row_f8_desc": "Toggles scan rate between idle (5 FPS) and boosted (60 FPS). In Lookahead window — skips warning (false alarm).",
      "hotkeys_row_ctrlv_title": "Timings Parsing",
      "hotkeys_row_ctrlv_desc": "Pastes schedule screenshot from clipboard for automatic WinRT OCR recognition into scheduler.",
      "hotkeys_row_modes_title": "Operation Modes",
      "hotkeys_row_modes_desc": "Quick toggle: 0 — Hybrid, 1 — Player Only, 2 — Screen Only, 3 — Standby (Off).",
      "hotkeys_row_mode_cycle_title": "Cycle Operation Mode",
      "hotkeys_row_mode_cycle_desc": "Cycle stream protection modes: 0 (Hybrid) ➔ 1 (Player Only) ➔ 2 (Screen Only) ➔ 3 (Standby / Off). Displays HUD notification on chosen monitor upon change.",
      "audit_header_title": "Censorship Incident Log",
      "audit_btn_clear_text": "Clear",
      "audit_btn_detach": "Pop out window",
      "audit_window_badge": "INCIDENT LOG (AUDIT)",
      "audit_window_title": "Censorship Incident & Audit Log",
      "audit_stat_total": "Total",
      "audit_stat_nsfw": "NSFW",
      "audit_stat_ocr": "Stopwords",
      "audit_stat_cues": "Timings",
      "audit_tab_all": "All",
      "audit_tab_nsfw": "Nudity",
      "audit_tab_ocr": "Stopwords",
      "audit_tab_cue": "Timings",
      "audit_tab_active": "Active",
      "audit_th_time": "Time",
      "audit_th_monitor": "Monitor",
      "audit_th_type": "Type",
      "audit_th_violation": "Violation",
      "audit_th_action": "Action in OBS",
      "audit_th_status": "Status",
      "audit_empty_row": "No incidents recorded. Stream is safe.",
      "mode_hud_title": "OPERATION MODE",
      "mode_name_0": "HYBRID MODE",
      "mode_name_1": "PLAYER ONLY",
      "mode_name_2": "SCREEN ONLY",
      "mode_name_3": "PROTECTION OFF (STANDBY)",
      "mode_desc_0": "Screen and media player protection active",
      "mode_desc_1": "Screen capture off, player lookahead active (CPU < 1%)",
      "mode_desc_2": "Direct screen capture & OCR active",
      "mode_desc_3": "Monitoring paused, censorship deactivated",
      "btn_pin_window": "Always on top",
      "btn_unpin_window": "Normal window mode",
      "btn_minimize_window": "Minimize",
      "btn_close_window": "Close",
      "hotkeys_scope_label": "Hotkey Execution Scope:",
      "hotkeys_scope_global": "Globally in Windows",
      "hotkeys_scope_global_desc": "Works across all games, browsers, and other windows",
      "hotkeys_scope_local": "Locally in application",
      "hotkeys_scope_local_desc": "Triggers only when the blewred window is active",
      "hotkeys_rebind_title": "Custom Keybindings",
      "hotkeys_rebind_desc": "Click on any hotkey badge and press the desired combination (e.g. Ctrl+Shift+F9):",
      "hotkeys_press_key": "Press key combination...",
      "hotkeys_btn_reset": "Defaults",
      "hotkeys_btn_save": "Save Hotkeys",
      "hotkeys_btn_record": "Record",
      "hotkeys_btn_recording": "Recording...",
      "hotkeys_recording_prompt": "Press key combination (e.g. Ctrl+Shift+F9)...",
      "hotkeys_release_hint": "release any key to save", "hotkeys_holding_prefix": "Holding: ",
      "hotkeys_opt_fn": "F-Keys (F1–F24)",
      "hotkeys_opt_digits": "Digits",
      "hotkeys_opt_letters": "Letters",
      "hotkeys_opt_special": "Special",
      "models_desc_directml": "blewred uses two DirectML neural network cascades for preemptive protection: vit_nsfw.onnx frame classifier and 640m.onnx anatomy detector.",
      "models_card1_title": "1. Frame Classifier (ViT NSFW)",
      "models_card2_title": "2. Anatomy Detector (NudeNet 640m)",
      "models_speed_label": "Download speed:",
      "models_eta_label": "Estimated time remaining:",
      "models_btn_start_download": "Download Models",
      "ext_desc_lead": "The extension intercepts online players (Kinobox, Alloha, Collaps, Rutube, etc.) ahead of playback before viewers see it, providing zero censorship latency and CPU < 1%.",
      "ext_btn_launch_browser": "Launch Browser with Extension",
      "ext_btn_open_folder": "Open Extension Folder",
      "ext_guide_panel_title": "Chromium Installation Guide (Chrome, Edge, Brave, Opera, Yandex):",
      "ext_step1_text": "Open browser extensions page: chrome://extensions or edge://extensions.",
      "ext_step2_text": "Turn on Developer mode toggle in the upper right corner.",
      "ext_step3_text": "Click the Load unpacked button.",
      "ext_step4_text": "In the file dialog, select the extensions/chrome folder from the app directory (button above opens it).",
      "ext_step5_text": "The blewred icon will appear in the extensions bar. During playback, the extension automatically activates the preemptive buffer.",
      "cues_autocensor_label": "Automatic Censorship:",
      "cues_autocensor_status_on": "Enabled (OBS + Audio)",
      "cues_autocensor_status_off": "Disabled (HUD Only)",
      "cues_notifications_label": "Pop-up Notifications (HUD):",
      "cues_notifications_status_on": "Enabled (HUD)",
      "cues_notifications_status_off": "Disabled",
      "cues_prewarn_label": "Pre-warning lead:",
      "unit_seconds": "sec",
      "cues_mode_label": "List Mode:",
      "cues_mode_append": "Append",
      "cues_mode_replace": "Replace",
      "cues_paste_panel_heading": "Paste Screenshot or Text with Timings",
      "cues_ocr_status_badge": "Windows Media OCR (Native / Ready)",
      "cues_dropzone_title": "Click here and press Ctrl+V to paste screenshot (or drag and drop file)",
      "cues_dropzone_sub": "Smart parser accepts any delimiters & formats: 34:69-1.34 (normalizes to 35:09 – 01:34:00), 12.30 - 14.15 (scene), colons, dots, dashes.",
      "cues_preview_title": "Screenshot Loaded",
      "cues_preview_meta": "Ready for recognition via Windows Media OCR",
      "btn_cue_reset": "Reset",
      "btn_cue_recognize_ocr": "Recognize via Windows OCR",
      "cues_manual_text_label": "Or paste/edit timing text manually:",
      "btn_cue_parse_manual": "Parse This Text",
      "cues_manual_placeholder": "Example: 12:45-14:20 Inappropriate dialogue\\n34:69-1.34 Dangerous scene",
      "cues_list_heading": "Scheduled Timings List",
      "btn_clear_all_cues": "Clear All",
      "cues_th_status": "Status",
      "cues_th_interval": "Interval",
      "cues_th_duration": "Duration",
      "cues_th_reason": "Description / Reason",
      "cues_th_action": "Action",
      "cues_empty_table_text": "No scheduled timings. Paste a screenshot with timecodes via Ctrl+V.",
      "cues_footer_sync": "Extension synchronization active (port 51789)",
      "obs_setup_status_label": "OBS Studio Status:",
      "obs_status_not_running": "OBS Not Running",
      "obs_prompt_desc1": "OBS Studio is currently not running or the connection has not yet been established.",
      "obs_prompt_desc2": "To automatically inject the plugin, create Censor Shield scenes, and configure shaders, open OBS Studio and then click Continue Auto-Setup.",
      "btn_launch_obs_text": "Launch OBS Studio",
      "obs_launch_manual_hint": "or open OBS manually",
      "obs_setup_in_progress": "Auto-setup in progress...",
      "btn_cancel_text": "Cancel",
      "btn_continue_setup_text": "Continue Auto-Setup",
      "shield_brand_alt": "blewred",
      "shield_badge_censor_active": "CENSOR ACTIVE",
      "shield_caption_detected": "DETECTED VIOLATION",
      "shield_target_nsfw": "UNACCEPTABLE CONTENT (NSFW)",
      "shield_prob_caption_text": "PROBABILITY",
      "checking": "Checking...",
      "disconnected": "Disconnected",
      "btn_close": "Close",
      "nav_tray_aria": "Minimize to tray",
      "modal_close_title": "Close blewred",
      "modal_close_lead": "You are about to close the application window. Choose your preferred action:",
      "modal_close_btn_tray": "Minimize to Tray",
      "modal_close_recommended": "Recommended",
      "modal_close_tray_desc": "blewred will keep running in the background: preemptive analysis, stopwords detection, and OBS protection stay active. You can reopen the window anytime from the system tray.",
      "modal_close_btn_exit": "Exit Completely",
      "modal_close_exit_desc": "Fully stop all protection services, unload AI cascade neural models, and cleanly terminate the blewred process.",
      "modal_close_remember": "Remember my choice and do not ask again",
      "modal_close_btn_cancel": "Cancel",
      "setting_close_action_label": "When clicking close button:",
      "setting_close_action_ask": "Always ask",
      "setting_close_action_tray": "Minimize to tray",
      "setting_close_action_exit": "Exit completely",
      "modal_disable_support_title": "Disable Support Banner",
      "modal_disable_support_p1": "This application is a free open-source project without ads and has an option to disable showing developer support on the splash screen.",
      "modal_disable_support_p2": "Development takes time and resources, and support in turn helps understand that they were not spent in vain.",
      "modal_disable_support_p3": "At your preference, you can remove the support block so stream viewers do not see it.",
      "modal_disable_support_btn_yes": "Yes",
      "modal_disable_support_btn_no": "No"
    }
  };

  let inMemoryLang = DEFAULT_LANG;
  const changeListeners = [];

  function getLanguage() {
    try {
      if (typeof localStorage !== 'undefined') {
        const stored = localStorage.getItem(STORAGE_KEY);
        if (stored && DICTIONARIES[stored]) {
          inMemoryLang = stored;
        }
      }
    } catch (e) { }
    return inMemoryLang;
  }

  function setLanguage(lang) {
    if (!DICTIONARIES[lang]) {
      console.warn(`[i18n] Unsupported language: ${lang}`);
      return;
    }
    inMemoryLang = lang;
    try {
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem(STORAGE_KEY, lang);
      }
    } catch (e) { }

    applyToDOM(lang);

    for (let i = 0; i < changeListeners.length; i++) {
      try {
        changeListeners[i](lang);
      } catch (err) {
        console.error('[i18n] Error in change listener:', err);
      }
    }
  }

  function t(key, fallbackOrParams, params) {
    const currentLang = getLanguage();
    const dict = DICTIONARIES[currentLang] || DICTIONARIES[DEFAULT_LANG];
    let val = dict ? dict[key] : undefined;

    let actualFallback = typeof fallbackOrParams === 'string' ? fallbackOrParams : key;
    let actualParams = typeof fallbackOrParams === 'object' ? fallbackOrParams : params;

    if (val === undefined) {
      val = DICTIONARIES[DEFAULT_LANG] ? DICTIONARIES[DEFAULT_LANG][key] : actualFallback;
    }
    if (val === undefined) {
      val = actualFallback;
    }

    if (actualParams && typeof actualParams === 'object') {
      return String(val).replace(/{(\w+)}/g, function (_, k) {
        return actualParams[k] !== undefined ? actualParams[k] : `{${k}}`;
      });
    }
    return val;
  }

  function onLanguageChange(fn) {
    if (typeof fn === 'function') {
      changeListeners.push(fn);
    }
  }

  function applyToDOM(lang) {
    const l = lang || getLanguage();
    if (typeof document === 'undefined') return;

    document.documentElement.lang = l;

    // 1. data-i18n for textContent
    const textEls = document.querySelectorAll('[data-i18n]');
    for (let i = 0; i < textEls.length; i++) {
      const el = textEls[i];
      const key = el.getAttribute('data-i18n');
      if (key) {
        const translation = t(key);
        if (translation && translation !== key) {
          el.textContent = translation;
        }
      }
    }

    // 2. data-i18n-html for innerHTML
    const htmlEls = document.querySelectorAll('[data-i18n-html]');
    for (let i = 0; i < htmlEls.length; i++) {
      const el = htmlEls[i];
      const key = el.getAttribute('data-i18n-html');
      if (key) {
        const translation = t(key);
        if (translation && translation !== key) {
          el.innerHTML = translation;
        }
      }
    }

    // 3. data-i18n-placeholder for input placeholders
    const placeholderEls = document.querySelectorAll('[data-i18n-placeholder]');
    for (let i = 0; i < placeholderEls.length; i++) {
      const el = placeholderEls[i];
      const key = el.getAttribute('data-i18n-placeholder');
      if (key) {
        el.placeholder = t(key);
      }
    }

    // 4. data-i18n-aria for aria-label
    const ariaEls = document.querySelectorAll('[data-i18n-aria]');
    for (let i = 0; i < ariaEls.length; i++) {
      const el = ariaEls[i];
      const key = el.getAttribute('data-i18n-aria');
      if (key) {
        el.setAttribute('aria-label', t(key));
      }
    }

    // 5. Update language switch group buttons active states
    const ruBtn = document.getElementById('btn-lang-ru');
    const enBtn = document.getElementById('btn-lang-en');
    if (ruBtn && enBtn) {
      if (l === 'ru') {
        ruBtn.classList.add('active');
        enBtn.classList.remove('active');
      } else {
        enBtn.classList.add('active');
        ruBtn.classList.remove('active');
      }
    }
  }

  // Listen to cross-window storage event for instant sync
  if (typeof window !== 'undefined') {
    window.addEventListener('storage', function (e) {
      if (e.key === STORAGE_KEY && e.newValue && DICTIONARIES[e.newValue]) {
        applyToDOM(e.newValue);
        for (let i = 0; i < changeListeners.length; i++) {
          try {
            changeListeners[i](e.newValue);
          } catch (err) { }
        }
      }
    });
  }

  return {
    getLanguage,
    setLanguage,
    t,
    applyToDOM,
    onLanguageChange,
    DICTIONARIES,
    SUPPORTED_LANGUAGES
  };
}));
