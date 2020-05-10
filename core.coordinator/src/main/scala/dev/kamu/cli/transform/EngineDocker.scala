/*
 * Copyright (c) 2018 kamu.dev
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

package dev.kamu.cli.transform

import dev.kamu.cli.WorkspaceLayout
import dev.kamu.cli.external.{DockerClient, DockerImages, DockerRunArgs}
import org.apache.hadoop.fs.{FileSystem, Path}
import org.apache.log4j.Level

class EngineDocker(
  fileSystem: FileSystem,
  logLevel: Level,
  dockerClient: DockerClient,
  image: String = DockerImages.SPARK
) extends Engine(fileSystem, logLevel) {

  protected override def submit(
    appClass: String,
    workspaceLayout: WorkspaceLayout,
    jars: Seq[Path],
    extraMounts: Seq[Path],
    loggingConfig: Path
  ): Unit = {
    val appVolumes = Map(
      loggingConfig -> new Path("/opt/spark/conf/log4j.properties")
    )

    val extraVolumes = extraMounts.map(p => (p, p)).toMap

    val jarVolumes = jars
      .map(p => (p, new Path("/opt/kamu/jars/" + p.getName)))
      .toMap

    val workspaceVolumes =
      Seq(workspaceLayout.kamuRootDir, workspaceLayout.localVolumeDir)
        .filter(fileSystem.exists)
        .map(p => (p, p))
        .toMap

    val submitArgs = List(
      "/opt/spark/bin/spark-submit",
      "--master=local[4]",
      s"--driver-memory=2g",
      "--conf",
      "spark.sql.warehouse.dir=/opt/spark-warehouse",
      s"--class=$appClass"
    ) ++ (
      if (jars.nonEmpty)
        Seq("--jars=" + jarVolumes.values.map(_.toUri.getPath).mkString(","))
      else
        Seq()
    ) ++ Seq(
      "/opt/kamu/engine.spark.jar"
    )

    logger.info("Starting Spark job")

    try {
      dockerClient.runShell(
        DockerRunArgs(
          image = image,
          volumeMap = appVolumes ++ workspaceVolumes ++ jarVolumes ++ extraVolumes
        ),
        submitArgs
      )
    } finally {
      // TODO: avoid this by setting up correct user inside the container
      logger.debug("Fixing file ownership")

      val unix = new com.sun.security.auth.module.UnixSystem()
      val chownArgs = Seq(
        "chown",
        "-R",
        s"${unix.getUid}:${unix.getGid}"
      ) ++ workspaceVolumes.values.map(_.toUri.getPath)

      dockerClient.runShell(
        DockerRunArgs(
          image = image,
          volumeMap = workspaceVolumes
        ),
        chownArgs
      )
    }
  }
}