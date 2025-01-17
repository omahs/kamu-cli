IMAGE_REPO = docker.io/kamudata
IMAGE_JUPYTER_TAG = 0.5.0

KAMU_VERSION = 0.105.0


.PHONY: jupyter
jupyter:
	docker build \
		-t $(IMAGE_REPO)/jupyter:$(IMAGE_JUPYTER_TAG) \
		jupyter/


.PHONY: jupyter-push
jupyter-push:
	docker push $(IMAGE_REPO)/jupyter:$(IMAGE_JUPYTER_TAG)


#########################################################################################


.PHONY: kamu-base
kamu-base:
	docker build \
		--build-arg KAMU_VERSION=$(KAMU_VERSION) \
		-t $(IMAGE_REPO)/kamu-base:$(KAMU_VERSION) \
		kamu-base/


.PHONY: kamu-base-push
kamu-base-push:
	docker push $(IMAGE_REPO)/kamu-base:$(KAMU_VERSION)


#########################################################################################


.PHONY: kamu-base-with-data
kamu-base-with-data:
	docker build \
		--build-arg KAMU_VERSION=$(KAMU_VERSION) \
		-t $(IMAGE_REPO)/kamu-base:latest-with-data \
		kamu-base-with-data/


.PHONY: kamu-base-with-data-push
kamu-base-with-data-push:
	docker push $(IMAGE_REPO)/kamu-base:latest-with-data


#########################################################################################


.PHONY: kamu-dev
kamu-dev:
	cd .. \
		&& cross build --target x86_64-unknown-linux-gnu --release \
		&& cp target/x86_64-unknown-linux-gnu/release/kamu-cli images/dev/kamu

	# Make sure to run first: cargo run --bin create-test-workspace
	cp -r ../examples/covid dev/workspace

	docker build \
		--build-arg KAMU_VERSION=$(KAMU_VERSION) \
		-t $(IMAGE_REPO)/kamu-base:dev \
		dev/

	rm -rf dev/kamu dev/workspace


.PHONY: kamu-dev-push
kamu-dev-push:
	docker push $(IMAGE_REPO)/kamu-base:dev