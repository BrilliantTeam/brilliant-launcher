package net.brilliantw.launcher

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import net.kdt.pojavlaunch.BrilliantLaunch

@InvokeArg
class LaunchArgs {
  lateinit var versionId: String
  lateinit var gameDir: String
  lateinit var username: String
  lateinit var uuid: String
  var accessToken: String = "0"
  var xuid: String = ""
  var language: String = "zh-TW"
  var jvmArgs: String = ""
  var join: String = ""
  var controls: String = ""
  var mobile: String = ""
}

@InvokeArg
class GameSettingsArgs {
  lateinit var json: String
}

@InvokeArg
class InstallerArgs {
  lateinit var versionId: String
  lateinit var javaArgs: String
}

@InvokeArg
class StageArgs {
  lateinit var uri: String
}

@InvokeArg
class OpenFileArgs {
  lateinit var path: String
}

@InvokeArg
class PathArgs {
  var path: String = ""
}

@InvokeArg
class KeepAliveArgs {
  var on: Boolean = false
  var title: String = ""
  var channel: String = ""
}

@InvokeArg
class LoginArgs {
  lateinit var url: String
  lateinit var redirect: String
  var title: String = ""
  var close: String = ""
}

@TauriPlugin
class GamePlugin(private val activity: Activity) : Plugin(activity) {
  private var keepingAlive = false

  @Command
  fun launch(invoke: Invoke) {
    val args = invoke.parseArgs(LaunchArgs::class.java)
    BrilliantLaunch.launch(activity, args.versionId, args.gameDir, args.username, args.uuid, args.accessToken, args.xuid, 0, args.language, args.jvmArgs, args.join, args.controls, args.mobile,
      object : BrilliantLaunch.Callback {
        override fun onStarted() = invoke.resolve()
        override fun onFailed(error: Throwable) = invoke.reject(error.message ?: error.toString())
      })
  }

  @Command
  fun keepAlive(invoke: Invoke) {
    val args = invoke.parseArgs(KeepAliveArgs::class.java)
    val service = Intent(activity, DownloadService::class.java)
      .putExtra(DownloadService.EXTRA_TITLE, args.title)
      .putExtra(DownloadService.EXTRA_CHANNEL, args.channel)
    try {
      synchronized(this) {
        if (args.on && !keepingAlive) {
          androidx.core.content.ContextCompat.startForegroundService(activity, service)
          keepingAlive = true
        } else if (!args.on && keepingAlive) {
          keepingAlive = false
          activity.startService(service.setAction(DownloadService.ACTION_STOP))
        }
      }
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject(e.message ?: e.toString())
    }
  }

  @Command
  fun webLogin(invoke: Invoke) {
    val args = invoke.parseArgs(LoginArgs::class.java)
    val intent = Intent(activity, LoginActivity::class.java)
      .putExtra(LoginActivity.EXTRA_URL, args.url)
      .putExtra(LoginActivity.EXTRA_REDIRECT, args.redirect)
      .putExtra(LoginActivity.EXTRA_TITLE, args.title)
      .putExtra(LoginActivity.EXTRA_CLOSE, args.close)
    startActivityForResult(invoke, intent, "webLoginResult")
  }

  @ActivityCallback
  fun webLoginResult(invoke: Invoke, result: ActivityResult) {
    invoke.resolve(JSObject().put("url", result.data?.getStringExtra(LoginActivity.EXTRA_RESULT)))
  }

  private val updates by lazy { com.google.android.play.core.appupdate.AppUpdateManagerFactory.create(activity) }

  @Command
  fun playUpdateInfo(invoke: Invoke) {
    updates.appUpdateInfo
      .addOnSuccessListener { info ->
        invoke.resolve(JSObject()
          .put("available", info.updateAvailability() == com.google.android.play.core.install.model.UpdateAvailability.UPDATE_AVAILABLE && info.isUpdateTypeAllowed(com.google.android.play.core.install.model.AppUpdateType.FLEXIBLE))
          .put("downloaded", info.installStatus() == com.google.android.play.core.install.model.InstallStatus.DOWNLOADED)
          .put("status", info.installStatus())
          .put("bytes", info.bytesDownloaded())
          .put("total", info.totalBytesToDownload()))
      }
      .addOnFailureListener { invoke.resolve(JSObject().put("available", false).put("downloaded", false)) }
  }

  @Command
  fun startPlayUpdate(invoke: Invoke) {
    updates.appUpdateInfo
      .addOnSuccessListener { info ->
        val options = com.google.android.play.core.appupdate.AppUpdateOptions.defaultOptions(com.google.android.play.core.install.model.AppUpdateType.FLEXIBLE)
        updates.startUpdateFlow(info, activity, options)
          .addOnSuccessListener { code -> invoke.resolve(JSObject().put("accepted", code == Activity.RESULT_OK)) }
          .addOnFailureListener { e -> invoke.reject(e.message ?: e.toString()) }
      }
      .addOnFailureListener { e -> invoke.reject(e.message ?: e.toString()) }
  }

  @Command
  fun completePlayUpdate(invoke: Invoke) {
    updates.completeUpdate()
      .addOnSuccessListener { invoke.resolve(JSObject()) }
      .addOnFailureListener { e -> invoke.reject(e.message ?: e.toString()) }
  }

  @Command
  fun exportGameSettings(invoke: Invoke) {
    try {
      invoke.resolve(JSObject().put("json", BrilliantLaunch.exportSettings(activity)))
    } catch (e: Exception) {
      invoke.reject(e.message ?: e.toString())
    }
  }

  @Command
  fun importGameSettings(invoke: Invoke) {
    try {
      invoke.resolve(JSObject().put("applied", BrilliantLaunch.importSettings(activity, invoke.parseArgs(GameSettingsArgs::class.java).json)))
    } catch (e: Exception) {
      invoke.reject(e.message ?: e.toString())
    }
  }

  @Command
  fun gameRunning(invoke: Invoke) {
    val manager = activity.getSystemService(android.app.ActivityManager::class.java)
    val running = manager.runningAppProcesses?.any { it.processName.endsWith(":game") } == true
    invoke.resolve(JSObject().put("running", running))
  }

  @Command
  fun runInstaller(invoke: Invoke) {
    val args = invoke.parseArgs(InstallerArgs::class.java)
    BrilliantLaunch.runInstaller(activity, args.versionId, args.javaArgs, object : BrilliantLaunch.Callback {
      override fun onStarted() = invoke.resolve()
      override fun onFailed(error: Throwable) = invoke.reject(error.message ?: error.toString())
    })
  }

  @Command
  fun stopInstaller(invoke: Invoke) {
    val manager = activity.getSystemService(android.app.ActivityManager::class.java)
    manager.runningAppProcesses?.filter { it.processName.endsWith(":gui_installer") }?.forEach { android.os.Process.killProcess(it.pid) }
    invoke.resolve()
  }

  @Command
  fun installerRunning(invoke: Invoke) {
    val manager = activity.getSystemService(android.app.ActivityManager::class.java)
    val running = manager.runningAppProcesses?.any { it.processName.endsWith(":gui_installer") } == true
    invoke.resolve(JSObject().put("running", running))
  }

  @Command
  fun resumeGame(invoke: Invoke) {
    val manager = activity.getSystemService(android.app.ActivityManager::class.java)
    val task = manager.appTasks.firstOrNull { it.taskInfo.baseActivity?.className == "net.kdt.pojavlaunch.MainActivity" }
    task?.moveToFront()
    invoke.resolve(JSObject().put("found", task != null))
  }

  @Command
  fun stopGame(invoke: Invoke) {
    val manager = activity.getSystemService(android.app.ActivityManager::class.java)
    manager.runningAppProcesses?.filter { it.processName.endsWith(":game") }?.forEach { android.os.Process.killProcess(it.pid) }
    invoke.resolve()
  }

  @Command
  fun openFile(invoke: Invoke) {
    try {
      val file = File(invoke.parseArgs(OpenFileArgs::class.java).path)
      val uri = androidx.core.content.FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)
      val mime = android.webkit.MimeTypeMap.getSingleton().getMimeTypeFromExtension(file.extension.lowercase()) ?: "text/plain"
      val view = Intent(Intent.ACTION_VIEW).setDataAndType(uri, mime).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      activity.startActivity(Intent.createChooser(view, null))
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject(e.message ?: e.toString())
    }
  }

  @Command
  fun stage(invoke: Invoke) {
    val uri = Uri.parse(invoke.parseArgs(StageArgs::class.java).uri)
    Thread {
      try {
        val resolver = activity.contentResolver
        val name = resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
          if (it.moveToFirst()) it.getString(0) else null
        } ?: "picked"
        val dir = File(activity.cacheDir, "picked/${System.nanoTime()}").apply { mkdirs() }
        val out = File(dir, File(name).name)
        resolver.openInputStream(uri)?.use { input -> out.outputStream().use { input.copyTo(it) } }
          ?: throw IllegalStateException("無法開啟選取的檔案")
        invoke.resolve(JSObject().put("path", out.absolutePath))
      } catch (e: Exception) {
        invoke.reject(e.message ?: e.toString())
      }
    }.start()
  }

  private fun storageGranted(): Boolean =
    if (android.os.Build.VERSION.SDK_INT >= 30) android.os.Environment.isExternalStorageManager()
    else androidx.core.content.ContextCompat.checkSelfPermission(activity, android.Manifest.permission.WRITE_EXTERNAL_STORAGE) == android.content.pm.PackageManager.PERMISSION_GRANTED

  @Command
  fun storageInfo(invoke: Invoke) {
    val volumes = app.tauri.plugin.JSArray()
    androidx.core.content.ContextCompat.getExternalFilesDirs(activity, null).filterNotNull().forEachIndexed { index, dir ->
      val root = dir.absolutePath.substringBefore("/Android/data/")
      val removable = try { android.os.Environment.isExternalStorageRemovable(dir) } catch (e: Exception) { index > 0 }
      volumes.put(JSObject().put("path", root).put("app", dir.absolutePath).put("removable", removable).put("free", dir.usableSpace).put("total", dir.totalSpace))
    }
    invoke.resolve(JSObject().put("granted", storageGranted()).put("volumes", volumes).put("internalFree", activity.filesDir.usableSpace))
  }

  @Command
  fun requestStorageAccess(invoke: Invoke) {
    try {
      if (android.os.Build.VERSION.SDK_INT >= 30) {
        val intent = Intent(android.provider.Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:${activity.packageName}"))
        try { activity.startActivity(intent) } catch (e: Exception) { activity.startActivity(Intent(android.provider.Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION)) }
      } else {
        androidx.core.app.ActivityCompat.requestPermissions(activity, arrayOf(android.Manifest.permission.WRITE_EXTERNAL_STORAGE, android.Manifest.permission.READ_EXTERNAL_STORAGE), 4107)
      }
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject(e.message ?: e.toString())
    }
  }

  @Command
  fun freeSpace(invoke: Invoke) {
    var dir: File? = File(invoke.parseArgs(PathArgs::class.java).path)
    while (dir != null && !dir.exists()) dir = dir.parentFile
    invoke.resolve(JSObject().put("free", dir?.usableSpace ?: 0L))
  }

  @Command
  fun installApk(invoke: Invoke) {
    try {
      val file = File(invoke.parseArgs(PathArgs::class.java).path)
      if (!file.isFile) throw IllegalStateException("找不到已下載的更新檔，請重新下載")
      if (android.os.Build.VERSION.SDK_INT >= 26 && !activity.packageManager.canRequestPackageInstalls()) {
        activity.startActivity(Intent(android.provider.Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${activity.packageName}")))
        invoke.resolve(JSObject().put("status", "permission"))
        return
      }
      val uri = androidx.core.content.FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)
      val install = Intent(Intent.ACTION_VIEW)
        .setDataAndType(uri, "application/vnd.android.package-archive")
        .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
      activity.startActivity(install)
      invoke.resolve(JSObject().put("status", "started"))
    } catch (e: Exception) {
      invoke.reject(e.message ?: e.toString())
    }
  }

  @Command
  fun takeShortcut(invoke: Invoke) {
    val action = MainActivity.pendingShortcut
    MainActivity.pendingShortcut = null
    invoke.resolve(JSObject().put("action", action ?: ""))
  }
}
